use serde::Serialize;
use tauri::State;

use crate::commands::logger::push_log;
use crate::state::AppState;
use ruststudy_core::domain::log::LogLevel;
use ruststudy_core::domain::service::{ServiceKind, ServiceOrigin, ServiceStatus};

#[derive(Debug, Clone, Serialize)]
pub struct ServiceInfo {
    pub id: String,
    pub kind: ServiceKind,
    pub display_name: String,
    pub version: String,
    pub variant: Option<String>,
    pub port: u16,
    pub status: ServiceStatus,
    pub install_path: String,
    /// "phpstudy" | "store" | "manual"
    pub origin: String,
}

fn origin_str(o: &ServiceOrigin) -> &'static str {
    match o {
        ServiceOrigin::PhpStudy => "phpstudy",
        ServiceOrigin::Store => "store",
        ServiceOrigin::Manual => "manual",
    }
}

fn to_info(s: &ruststudy_core::domain::service::ServiceInstance) -> ServiceInfo {
    ServiceInfo {
        id: s.id(),
        kind: s.kind,
        display_name: format!("{} {}", s.kind.display_name(), s.version),
        version: s.version.clone(),
        variant: s.variant.clone(),
        port: s.port,
        status: s.status.clone(),
        install_path: s.install_path.display().to_string(),
        origin: origin_str(&s.origin).to_string(),
    }
}

fn all_infos(services: &[ruststudy_core::domain::service::ServiceInstance]) -> Vec<ServiceInfo> {
    services.iter().map(to_info).collect()
}

/// 后台并行刷新所有服务状态。single-flight：已有任务在跑时直接跳过。
/// 先在不持锁的情况下并行跑 status()，最后只短暂拿 write 锁写字段，避免锁阻塞。
pub async fn refresh_all_services_bg(state: AppState) {
    use std::sync::atomic::Ordering;
    if state
        .refresh_in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return; // 已有刷新任务在跑
    }
    // 1) 克隆一份 instances 用于并行探测（不持锁）
    let snapshot = { state.services.read().await.clone() };
    // 2) 并行跑所有 status()
    let results = futures_util::future::join_all(snapshot.iter().map(|inst| {
        let mgr = &state.service_manager;
        async move { (inst.id(), mgr.refresh_status_value(inst).await) }
    }))
    .await;
    // 3) 短暂拿 write 锁，只更新 status 字段
    {
        let mut services = state.services.write().await;
        for (id, status) in results {
            if let Some(s) = services.iter_mut().find(|s| s.id() == id) {
                if let Ok(st) = status {
                    s.status = st;
                }
            }
        }
    }
    state.refresh_in_flight.store(false, Ordering::Release);
}

#[tauri::command]
pub async fn get_services(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    // 快速路径：读锁取当前内存快照立即返回；后台异步刷新 status
    let snapshot = { state.services.read().await.clone() };
    let bg_state = state.inner().clone_shallow();
    tauri::async_runtime::spawn(async move {
        refresh_all_services_bg(bg_state).await;
    });
    Ok(all_infos(&snapshot))
}

/// 阻塞版：供"手动刷新"按钮或诊断使用，等所有 status 都刷新完再返回
#[tauri::command]
pub async fn get_services_fresh(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    refresh_all_services_bg(state.inner().clone_shallow()).await;
    let services = state.services.read().await;
    Ok(all_infos(&services))
}

/// 历史兼容：保留旧阻塞语义的实现，暂不暴露给前端；留作参考或回退
#[allow(dead_code)]
async fn get_services_legacy(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    let mut services = state.services.write().await;
    for instance in services.iter_mut() {
        let _ = state.service_manager.refresh_status(instance).await;
    }
    Ok(all_infos(&services))
}

#[tauri::command]
pub async fn start_service(id: String, state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    let mut services = state.services.write().await;

    // Find target index
    let idx = services
        .iter()
        .position(|s| s.id() == id)
        .ok_or_else(|| format!("Service not found: {}", id))?;

    // Split: take the target out, pass the rest for dependency resolution
    let mut target = services[idx].clone();
    let mut others: Vec<_> = services
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != idx)
        .map(|(_, s)| s.clone())
        .collect();

    let name = format!("{} {}", target.kind.display_name(), target.version);
    push_log(&state, LogLevel::Info, "service", format!("启动 {}", name), None, None).await;

    match state.service_manager.start_with_deps(&mut target, &mut others).await {
        Ok(_) => {
            let pid = match &target.status { ServiceStatus::Running { pid } => *pid, _ => 0 };
            push_log(&state, LogLevel::Success, "service", format!("{} 启动成功（PID {}）", name, pid), None, None).await;
        }
        Err(e) => {
            let msg = e.to_string();
            push_log(&state, LogLevel::Error, "service", format!("{} 启动失败", name), Some(msg.clone()), None).await;
            return Err(msg);
        }
    }

    // Write back
    services[idx] = target;
    for (i, svc) in services.iter_mut().enumerate() {
        if i != idx {
            // Find matching service in others by id
            if let Some(updated) = others.iter().find(|o| o.id() == svc.id()) {
                svc.status = updated.status.clone();
            }
        }
    }

    Ok(all_infos(&services))
}

#[tauri::command]
pub async fn stop_service(id: String, state: State<'_, AppState>) -> Result<ServiceInfo, String> {
    let mut services = state.services.write().await;
    let instance = services
        .iter_mut()
        .find(|s| s.id() == id)
        .ok_or_else(|| format!("Service not found: {}", id))?;

    let name = format!("{} {}", instance.kind.display_name(), instance.version);
    push_log(&state, LogLevel::Info, "service", format!("停止 {}", name), None, None).await;

    match state.service_manager.stop_service(instance).await {
        Ok(_) => push_log(&state, LogLevel::Success, "service", format!("{} 已停止", name), None, None).await,
        Err(e) => {
            let msg = e.to_string();
            push_log(&state, LogLevel::Error, "service", format!("{} 停止失败", name), Some(msg.clone()), None).await;
            return Err(msg);
        }
    }

    Ok(to_info(instance))
}

#[tauri::command]
pub async fn restart_service(
    id: String,
    state: State<'_, AppState>,
) -> Result<ServiceInfo, String> {
    let mut services = state.services.write().await;
    let instance = services
        .iter_mut()
        .find(|s| s.id() == id)
        .ok_or_else(|| format!("Service not found: {}", id))?;

    let name = format!("{} {}", instance.kind.display_name(), instance.version);
    push_log(&state, LogLevel::Info, "service", format!("重启 {}", name), None, None).await;

    match state.service_manager.restart_service(instance).await {
        Ok(_) => push_log(&state, LogLevel::Success, "service", format!("{} 已重启", name), None, None).await,
        Err(e) => {
            let msg = e.to_string();
            push_log(&state, LogLevel::Error, "service", format!("{} 重启失败", name), Some(msg.clone()), None).await;
            return Err(msg);
        }
    }

    Ok(to_info(instance))
}

#[tauri::command]
pub async fn start_all(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    let mut services = state.services.write().await;
    let config = state.config.read().await;
    let active_web = config.web_server.active.clone();
    drop(config);

    for instance in services.iter_mut() {
        // Skip the non-active web server (only start Nginx OR Apache, not both)
        match instance.kind {
            ServiceKind::Nginx if active_web != "nginx" => continue,
            ServiceKind::Apache if active_web != "apache" => continue,
            _ => {}
        }
        let _ = state.service_manager.start_service(instance).await;
    }
    Ok(all_infos(&services))
}

#[tauri::command]
pub async fn stop_all(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    let mut services = state.services.write().await;
    for instance in services.iter_mut() {
        let _ = state.service_manager.stop_service(instance).await;
    }
    Ok(all_infos(&services))
}
