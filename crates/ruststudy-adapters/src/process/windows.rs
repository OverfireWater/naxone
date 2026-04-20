use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};

use ruststudy_core::domain::service::{ServiceInstance, ServiceKind, ServiceStatus};
use ruststudy_core::error::{Result, RustStudyError};
use ruststudy_core::ports::process::ProcessManager;

struct ProcessInfo {
    pid: u32,
}

/// status 缓存 TTL：低于前端 5s 轮询间隔，用户操作后下一轮就能看到变化
const STATUS_CACHE_TTL: Duration = Duration::from_millis(1000);
/// netstat snapshot 的合并窗口：同一轮 refresh_all 内所有 status 复用一份
const NETSTAT_SNAPSHOT_TTL: Duration = Duration::from_millis(500);

pub struct WindowsProcessManager {
    processes: Arc<RwLock<HashMap<String, ProcessInfo>>>,
    /// 按 instance.id() 缓存上次 status 结果，降低重复探测成本
    status_cache: Arc<RwLock<HashMap<String, (Instant, ServiceStatus)>>>,
    /// 最近一次 netstat 结果（port → pid），短 TTL 合并同窗口重复调用
    netstat_cache: Arc<Mutex<Option<(Instant, HashMap<u16, u32>)>>>,
}

impl WindowsProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            status_cache: Arc::new(RwLock::new(HashMap::new())),
            netstat_cache: Arc::new(Mutex::new(None)),
        }
    }

    /// 一次 `netstat -ano` 把所有 LISTENING 行的 port→pid 全抓出来，供并行 status 复用
    pub async fn snapshot_listening_ports(&self) -> HashMap<u16, u32> {
        // 命中未过期缓存 → 直接返回
        {
            let guard = self.netstat_cache.lock().await;
            if let Some((ts, map)) = guard.as_ref() {
                if ts.elapsed() < NETSTAT_SNAPSHOT_TTL {
                    return map.clone();
                }
            }
        }
        // 重新扫
        let map = netstat_snapshot().await;
        let mut guard = self.netstat_cache.lock().await;
        *guard = Some((Instant::now(), map.clone()));
        map
    }

    /// 失效单个服务的 status 缓存（在 start/stop/restart 后调用，避免返回 stale 数据）
    async fn invalidate_status(&self, instance_id: &str) {
        self.status_cache.write().await.remove(instance_id);
        // netstat 快照也过期掉，下次刷新重新扫
        *self.netstat_cache.lock().await = None;
    }

    fn build_start_command(instance: &ServiceInstance) -> Result<Command> {
        let install_path = &instance.install_path;

        match instance.kind {
            ServiceKind::Nginx => {
                let exe = install_path.join("nginx.exe");
                let conf = instance
                    .config_path
                    .clone()
                    .unwrap_or_else(|| install_path.join("conf").join("nginx.conf"));
                let mut cmd = Command::new(&exe);
                cmd.arg("-c").arg(&conf);
                cmd.current_dir(install_path);
                Ok(cmd)
            }
            ServiceKind::Apache => {
                let exe = install_path.join("bin").join("httpd.exe");
                let conf = instance
                    .config_path
                    .clone()
                    .unwrap_or_else(|| install_path.join("conf").join("httpd.conf"));
                let mut cmd = Command::new(&exe);
                cmd.arg("-f").arg(&conf);
                cmd.current_dir(install_path);
                Ok(cmd)
            }
            ServiceKind::Php => {
                let exe = install_path.join("php-cgi.exe");
                let bind = format!("127.0.0.1:{}", instance.port);
                let mut cmd = Command::new(&exe);
                cmd.arg("-b").arg(&bind);
                if let Some(conf) = &instance.config_path {
                    cmd.arg("-c").arg(conf);
                }
                cmd.current_dir(install_path);
                Ok(cmd)
            }
            ServiceKind::Mysql => {
                let exe = install_path.join("bin").join("mysqld.exe");
                let conf = instance
                    .config_path
                    .clone()
                    .unwrap_or_else(|| install_path.join("my.ini"));
                let mut cmd = Command::new(&exe);
                cmd.arg(format!("--defaults-file={}", conf.display()));
                cmd.current_dir(install_path);
                Ok(cmd)
            }
            ServiceKind::Redis => {
                let exe = install_path.join("redis-server.exe");
                let conf = instance
                    .config_path
                    .clone()
                    .unwrap_or_else(|| install_path.join("redis.windows.conf"));
                let mut cmd = Command::new(&exe);
                cmd.arg(&conf);
                cmd.current_dir(install_path);
                Ok(cmd)
            }
        }
    }

    fn build_stop_command(instance: &ServiceInstance) -> Option<Command> {
        let install_path = &instance.install_path;

        match instance.kind {
            ServiceKind::Nginx => {
                let exe = install_path.join("nginx.exe");
                let mut cmd = Command::new(&exe);
                cmd.arg("-s").arg("quit");
                cmd.current_dir(install_path);
                Some(cmd)
            }
            ServiceKind::Apache => {
                let exe = install_path.join("bin").join("httpd.exe");
                let mut cmd = Command::new(&exe);
                cmd.arg("-k").arg("stop");
                cmd.current_dir(install_path);
                Some(cmd)
            }
            ServiceKind::Redis => {
                let exe = install_path.join("redis-cli.exe");
                let mut cmd = Command::new(&exe);
                cmd.arg("-p")
                    .arg(instance.port.to_string())
                    .arg("shutdown");
                Some(cmd)
            }
            ServiceKind::Mysql => {
                let exe = install_path.join("bin").join("mysqladmin.exe");
                let mut cmd = Command::new(&exe);
                cmd.arg("-u").arg("root").arg("shutdown");
                cmd.current_dir(install_path);
                Some(cmd)
            }
            // PHP-CGI: no graceful shutdown, kill by PID
            ServiceKind::Php => None,
        }
    }

    /// 真正的状态探测（无缓存）。
    /// 若已有 netstat snapshot 则复用，否则只做 port probe + 必要时才 fallback 到 netstat。
    async fn probe_status(&self, instance: &ServiceInstance) -> ServiceStatus {
        // 1) 快速 TCP 探针
        if !probe_port(instance.port).await {
            return ServiceStatus::Stopped;
        }

        // 2) 优先信任自己启动的 PID
        {
            let procs = self.processes.read().await;
            if let Some(info) = procs.get(&instance.id()) {
                return ServiceStatus::Running { pid: info.pid };
            }
        }

        // 3) 从 netstat snapshot 查 PID（比单个 netstat 快且复用）
        let snapshot = self.snapshot_listening_ports().await;
        let pid = snapshot.get(&instance.port).copied().unwrap_or(0);
        if pid == 0 {
            return ServiceStatus::Stopped;
        }

        // 4) PHP 信任端口；web 服务器需按进程名校验避开 nginx/apache 混淆
        if instance.kind == ServiceKind::Php {
            return ServiceStatus::Running { pid };
        }
        if pid_matches_service(pid, instance.kind).await {
            return ServiceStatus::Running { pid };
        }
        ServiceStatus::Stopped
    }
}

#[async_trait]
impl ProcessManager for WindowsProcessManager {
    async fn start(&self, instance: &ServiceInstance) -> Result<u32> {
        let mut cmd = Self::build_start_command(instance)?;

        let mut child = cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                RustStudyError::Process(format!(
                    "无法启动 {}: {}",
                    instance.kind.display_name(),
                    e
                ))
            })?;

        let pid = child.id().unwrap_or(0);

        // For short-lived processes (Nginx/Apache may exit quickly on config error),
        // wait briefly and check if process is still alive
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Check if process exited immediately (config error etc.)
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                // Process exited with error, read stderr for details
                let stderr = if let Some(mut err) = child.stderr.take() {
                    let mut buf = String::new();
                    use tokio::io::AsyncReadExt;
                    let _ = err.read_to_string(&mut buf).await;
                    buf
                } else {
                    String::new()
                };
                let code = status.code().unwrap_or(-1);
                let msg = if stderr.trim().is_empty() {
                    format!("{} 启动失败 (exit code {})", instance.kind.display_name(), code)
                } else {
                    format!("{} 启动失败: {}", instance.kind.display_name(), stderr.trim().lines().last().unwrap_or("unknown error"))
                };
                return Err(RustStudyError::Process(msg));
            }
            _ => {} // Still running or exited successfully
        }

        let mut procs = self.processes.write().await;
        procs.insert(instance.id(), ProcessInfo { pid });
        drop(procs);

        // 主动失效 status 缓存，下次 status() 重新探测得到 Running
        self.invalidate_status(&instance.id()).await;

        tracing::info!(
            service = instance.kind.display_name(),
            pid = pid,
            "Service started"
        );

        Ok(pid)
    }

    async fn stop(&self, instance: &ServiceInstance) -> Result<()> {
        // Try graceful stop command first (nginx -s quit, httpd -k stop, redis-cli shutdown)
        if let Some(mut cmd) = Self::build_stop_command(instance) {
            let result = cmd
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await;

            if let Ok(status) = result {
                if status.success() {
                    let mut procs = self.processes.write().await;
                    procs.remove(&instance.id());
                    tracing::info!(
                        service = instance.kind.display_name(),
                        "Service stopped gracefully"
                    );
                    return Ok(());
                }
            }
        }

        // Fallback: find PID by port and kill it
        let pid = {
            let procs = self.processes.read().await;
            if let Some(info) = procs.get(&instance.id()) {
                info.pid
            } else {
                // Not tracked by us, find PID via port (e.g. PHPStudy started it)
                find_pid_by_port(instance.port).await
            }
        };

        if pid > 0 {
            let kill = Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .output()
                .await;
            // taskkill 失败（常见：外部管理员进程 Access is denied）→ 上报而非静默
            if let Ok(out) = &kill {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    let hint = if stderr.contains("Access is denied") || stderr.contains("拒绝访问")
                    {
                        format!(
                            "无法结束 {} (PID {}): 权限不足（进程可能以管理员身份启动）",
                            instance.kind.display_name(),
                            pid
                        )
                    } else {
                        format!(
                            "无法结束 {} (PID {}): {}",
                            instance.kind.display_name(),
                            pid,
                            stderr
                        )
                    };
                    return Err(RustStudyError::Process(hint));
                }
            }
        }

        // 最后确认端口真的释放了（覆盖 pid=0 但端口仍被占用的情况，例如多个同类残留进程）
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if probe_port(instance.port).await {
            // 再探一次外部 PID，能拿到就一起报出去
            let still_pid = find_pid_by_port(instance.port).await;
            let extra = if still_pid > 0 {
                format!("（端口仍被 PID {} 占用）", still_pid)
            } else {
                String::new()
            };
            return Err(RustStudyError::Process(format!(
                "{} 已发出停止命令但端口 {} 仍被占用{}",
                instance.kind.display_name(),
                instance.port,
                extra
            )));
        }

        let mut procs = self.processes.write().await;
        procs.remove(&instance.id());
        drop(procs);

        // 主动失效 status 缓存
        self.invalidate_status(&instance.id()).await;

        tracing::info!(
            service = instance.kind.display_name(),
            pid = pid,
            "Service stopped"
        );
        Ok(())
    }

    async fn restart(&self, instance: &ServiceInstance) -> Result<u32> {
        self.stop(instance).await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let pid = self.start(instance).await?;
        // start/stop 已各自 invalidate 过，这里确保 restart 结束后缓存干净
        self.invalidate_status(&instance.id()).await;
        Ok(pid)
    }

    async fn status(&self, instance: &ServiceInstance) -> Result<ServiceStatus> {
        // 命中有效缓存 → 直接返回，省掉 netstat/tasklist
        {
            let cache = self.status_cache.read().await;
            if let Some((ts, st)) = cache.get(&instance.id()) {
                if ts.elapsed() < STATUS_CACHE_TTL {
                    return Ok(st.clone());
                }
            }
        }

        let status = self.probe_status(instance).await;

        // 写回缓存
        self.status_cache
            .write()
            .await
            .insert(instance.id(), (Instant::now(), status.clone()));

        Ok(status)
    }

    async fn reload(&self, instance: &ServiceInstance) -> Result<()> {
        match instance.kind {
            ServiceKind::Nginx => {
                // First run -t to test config
                let exe = instance.install_path.join("nginx.exe");
                let test = Command::new(&exe)
                    .arg("-t")
                    .current_dir(&instance.install_path)
                    .output()
                    .await
                    .map_err(|e| RustStudyError::Process(format!("Nginx 测试失败: {e}")))?;
                if !test.status.success() {
                    let stderr = String::from_utf8_lossy(&test.stderr);
                    let msg = stderr.lines().find(|l| l.contains("emerg") || l.contains("error"))
                        .unwrap_or_else(|| stderr.lines().last().unwrap_or("unknown")).to_string();
                    return Err(RustStudyError::Process(format!("Nginx 配置错误: {}", msg)));
                }
                // Then reload
                let reload_out = Command::new(&exe)
                    .arg("-s").arg("reload")
                    .current_dir(&instance.install_path)
                    .output()
                    .await
                    .map_err(|e| RustStudyError::Process(format!("Nginx reload 失败: {e}")))?;
                if !reload_out.status.success() {
                    let stderr = String::from_utf8_lossy(&reload_out.stderr);
                    return Err(RustStudyError::Process(format!("Nginx reload 失败: {}", stderr.trim())));
                }
            }
            ServiceKind::Apache => {
                let exe = instance.install_path.join("bin").join("httpd.exe");
                let out = Command::new(&exe)
                    .arg("-k").arg("graceful")
                    .current_dir(&instance.install_path)
                    .output()
                    .await
                    .map_err(|e| RustStudyError::Process(format!("Apache reload 失败: {e}")))?;
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(RustStudyError::Process(format!("Apache reload 失败: {}", stderr.trim())));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Quick TCP port probe with 300ms timeout
async fn probe_port(port: u16) -> bool {
    tokio::time::timeout(
        std::time::Duration::from_millis(300),
        tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port)),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}

/// Find the PID of the process listening on a given port using `netstat`
async fn find_pid_by_port(port: u16) -> u32 {
    netstat_snapshot().await.get(&port).copied().unwrap_or(0)
}

/// 一次 netstat 调用解析出所有 LISTENING 端口 → PID 映射
async fn netstat_snapshot() -> HashMap<u16, u32> {
    let mut map = HashMap::new();
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await;
    let Ok(output) = output else { return map };
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        if !line.contains("LISTENING") {
            continue;
        }
        // 格式: "  TCP    0.0.0.0:80    0.0.0.0:0    LISTENING    1234"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        let local = parts[1];
        let Some(colon) = local.rfind(':') else { continue };
        let Ok(port) = local[colon + 1..].parse::<u16>() else { continue };
        let Ok(pid) = parts[parts.len() - 1].parse::<u32>() else { continue };
        // 同一端口可能出现多条（IPv4/IPv6），保留非零 pid 的第一条
        map.entry(port).or_insert(pid);
    }
    map
}

/// Get the executable name of a process by PID using `tasklist`
async fn get_process_name(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    // Output: "nginx.exe","1234","Console","1","12,345 K"
    let first_line = text.lines().next()?;
    let name = first_line.split(',').next()?;
    Some(name.trim_matches('"').to_lowercase())
}

/// Check if a PID belongs to a specific service kind
async fn pid_matches_service(pid: u32, kind: ServiceKind) -> bool {
    if pid == 0 {
        return false;
    }
    let Some(name) = get_process_name(pid).await else {
        return false;
    };
    match kind {
        ServiceKind::Nginx => name.contains("nginx"),
        ServiceKind::Apache => name.contains("httpd"),
        ServiceKind::Php => name.contains("php"),
        ServiceKind::Mysql => name.contains("mysqld"),
        ServiceKind::Redis => name.contains("redis"),
    }
}
