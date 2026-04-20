use std::sync::Arc;

use crate::domain::service::{ServiceInstance, ServiceKind, ServiceStatus};
use crate::error::{Result, RustStudyError};
use crate::ports::process::ProcessManager;

/// Orchestrates service lifecycle operations
#[derive(Clone)]
pub struct ServiceManager {
    process_mgr: Arc<dyn ProcessManager>,
}

impl ServiceManager {
    pub fn new(process_mgr: Arc<dyn ProcessManager>) -> Self {
        Self { process_mgr }
    }

    pub async fn start_service(&self, instance: &mut ServiceInstance) -> Result<()> {
        if instance.status.is_running() {
            return Ok(());
        }
        instance.status = ServiceStatus::Starting;
        let pid = self.process_mgr.start(instance).await?;
        instance.status = ServiceStatus::Running { pid };
        Ok(())
    }

    pub async fn stop_service(&self, instance: &mut ServiceInstance) -> Result<()> {
        if !instance.status.is_running() {
            return Ok(());
        }
        instance.status = ServiceStatus::Stopping;
        self.process_mgr.stop(instance).await?;
        instance.status = ServiceStatus::Stopped;
        Ok(())
    }

    pub async fn restart_service(&self, instance: &mut ServiceInstance) -> Result<()> {
        self.stop_service(instance).await?;
        self.start_service(instance).await?;
        Ok(())
    }

    pub async fn refresh_status(&self, instance: &mut ServiceInstance) -> Result<()> {
        instance.status = self.process_mgr.status(instance).await?;
        Ok(())
    }

    /// 只读版：返回最新 status，不修改 instance。供只读快照 + 后台并行刷新使用。
    pub async fn refresh_status_value(&self, instance: &ServiceInstance) -> Result<ServiceStatus> {
        self.process_mgr.status(instance).await
    }

    /// Start a service with dependency and mutual-exclusion awareness:
    /// - Nginx and Apache are mutually exclusive (both use port 80), starting one stops the other
    /// - Starting Nginx/Apache also auto-starts PHP-CGI instances
    pub async fn start_with_deps(
        &self,
        target: &mut ServiceInstance,
        all_services: &mut [ServiceInstance],
    ) -> Result<()> {
        let is_web_server = matches!(target.kind, ServiceKind::Nginx | ServiceKind::Apache);

        if is_web_server {
            // Mutual exclusion: stop the other web server if running (both use port 80).
            // 若停对方失败或实际未停止 → 中止启动，避免 bind 冲突产生误导性 AH00015。
            let rival_kind = match target.kind {
                ServiceKind::Nginx => ServiceKind::Apache,
                _ => ServiceKind::Nginx,
            };
            for svc in all_services.iter_mut() {
                if svc.kind == rival_kind && svc.status.is_running() {
                    if let Err(e) = self.stop_service(svc).await {
                        return Err(RustStudyError::Process(format!(
                            "无法停止 {}（占用端口 80）: {}",
                            rival_kind.display_name(),
                            e
                        )));
                    }
                    // 再验证一次：确认对方确实不在 running 状态
                    // ProcessManager::status 内部已用 TCP probe + 超时，不需要再加延时
                    if let Ok(status) = self.process_mgr.status(svc).await {
                        svc.status = status.clone();
                    }
                    if svc.status.is_running() {
                        return Err(RustStudyError::Process(format!(
                            "{} 停止后仍在运行（可能是以管理员身份启动的外部进程），已中止启动 {}",
                            rival_kind.display_name(),
                            target.kind.display_name()
                        )));
                    }
                }
            }
        }

        // Start the target service itself
        self.start_service(target).await?;

        // If it's a web server, auto-start PHP-CGI instances
        if is_web_server {
            for svc in all_services.iter_mut() {
                if svc.kind == ServiceKind::Php && !svc.status.is_running() {
                    let _ = self.start_service(svc).await;
                }
            }
        }

        Ok(())
    }
}
