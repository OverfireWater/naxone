use serde::Serialize;
use tauri::State;

use crate::commands::logger::push_log;
use crate::state::AppState;
use naxone_core::domain::log::LogLevel;
use naxone_core::domain::service::{
    ServiceInstance, ServiceKind, ServiceOrigin, ServiceStatus,
};

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
        ServiceOrigin::System => "system",
    }
}

fn to_info(s: &ServiceInstance) -> ServiceInfo {
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

fn all_infos(services: &[ServiceInstance]) -> Vec<ServiceInfo> {
    services.iter().map(to_info).collect()
}

fn find_service_index(services: &[ServiceInstance], id: &str) -> Result<usize, String> {
    services
        .iter()
        .position(|s| s.id() == id)
        .ok_or_else(|| format!("Service not found: {}", id))
}

fn sync_updated_statuses(
    services: &mut [ServiceInstance],
    target: &ServiceInstance,
    others: &[ServiceInstance],
) {
    for svc in services.iter_mut() {
        if svc.id() == target.id() {
            *svc = target.clone();
            continue;
        }
        if let Some(updated) = others.iter().find(|o| o.id() == svc.id()) {
            svc.status = updated.status.clone();
        }
    }
}

fn sync_all_statuses(dest: &mut [ServiceInstance], src: &[ServiceInstance]) {
    for svc in dest.iter_mut() {
        if let Some(updated) = src.iter().find(|s| s.id() == svc.id()) {
            svc.status = updated.status.clone();
        }
    }
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
        return;
    }

    let snapshot = { state.services.read().await.clone() };
    let results = futures_util::future::join_all(snapshot.iter().map(|inst| {
        let mgr = &state.service_manager;
        async move { (inst.id(), mgr.refresh_status_value(inst).await) }
    }))
    .await;

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
    let snapshot = { state.services.read().await.clone() };
    let bg_state = state.inner().clone_shallow();
    tauri::async_runtime::spawn(async move {
        refresh_all_services_bg(bg_state).await;
    });
    Ok(all_infos(&snapshot))
}

#[tauri::command]
pub async fn get_services_fresh(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    refresh_all_services_bg(state.inner().clone_shallow()).await;
    let services = state.services.read().await;
    Ok(all_infos(&services))
}

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
    let snapshot = { state.services.read().await.clone() };
    let idx = find_service_index(&snapshot, &id)?;
    let mut target = snapshot[idx].clone();
    let mut others: Vec<_> = snapshot
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != idx)
        .map(|(_, s)| s.clone())
        .collect();

    let name = format!("{} {}", target.kind.display_name(), target.version);
    push_log(&state, LogLevel::Info, "service", format!("启动 {}", name), None, None).await;

    match state.service_manager.start_with_deps(&mut target, &mut others).await {
        Ok(_) => {
            let pid = match &target.status {
                ServiceStatus::Running { pid, .. } => *pid,
                _ => 0,
            };
            push_log(
                &state,
                LogLevel::Success,
                "service",
                format!("{} 启动成功（PID {}）", name, pid),
                None,
                None,
            )
            .await;
        }
        Err(e) => {
            let msg = e.to_string();
            push_log(
                &state,
                LogLevel::Error,
                "service",
                format!("{} 启动失败", name),
                Some(msg.clone()),
                None,
            )
            .await;
            return Err(msg);
        }
    }

    let mut services = state.services.write().await;
    sync_updated_statuses(&mut services, &target, &others);
    Ok(all_infos(&services))
}

#[tauri::command]
pub async fn stop_service(id: String, state: State<'_, AppState>) -> Result<ServiceInfo, String> {
    let snapshot = { state.services.read().await.clone() };
    let idx = find_service_index(&snapshot, &id)?;
    let mut target = snapshot[idx].clone();

    let name = format!("{} {}", target.kind.display_name(), target.version);
    push_log(&state, LogLevel::Info, "service", format!("停止 {}", name), None, None).await;

    match state.service_manager.stop_service(&mut target).await {
        Ok(_) => {
            push_log(
                &state,
                LogLevel::Success,
                "service",
                format!("{} 已停止", name),
                None,
                None,
            )
            .await;
        }
        Err(e) => {
            let msg = e.to_string();
            push_log(
                &state,
                LogLevel::Error,
                "service",
                format!("{} 停止失败", name),
                Some(msg.clone()),
                None,
            )
            .await;
            return Err(msg);
        }
    }

    let mut services = state.services.write().await;
    if let Some(svc) = services.iter_mut().find(|s| s.id() == target.id()) {
        *svc = target.clone();
    }
    Ok(to_info(&target))
}

#[tauri::command]
pub async fn restart_service(
    id: String,
    state: State<'_, AppState>,
) -> Result<ServiceInfo, String> {
    let snapshot = { state.services.read().await.clone() };
    let idx = find_service_index(&snapshot, &id)?;

    let mut target = snapshot[idx].clone();
    let mut others: Vec<_> = snapshot
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != idx)
        .map(|(_, s)| s.clone())
        .collect();

    let name = format!("{} {}", target.kind.display_name(), target.version);
    push_log(&state, LogLevel::Info, "service", format!("重启 {}", name), None, None).await;

    let result = if matches!(target.kind, ServiceKind::Nginx | ServiceKind::Apache) {
        if let Err(e) = state.service_manager.stop_service(&mut target).await {
            Err(e)
        } else {
            state
                .service_manager
                .start_with_deps(&mut target, &mut others)
                .await
        }
    } else {
        state.service_manager.restart_service(&mut target).await
    };

    if let Err(e) = result {
        let msg = e.to_string();
        push_log(
            &state,
            LogLevel::Error,
            "service",
            format!("{} 重启失败", name),
            Some(msg.clone()),
            None,
        )
        .await;
        return Err(msg);
    }

    push_log(
        &state,
        LogLevel::Success,
        "service",
        format!("{} 已重启", name),
        None,
        None,
    )
    .await;

    let mut services = state.services.write().await;
    sync_updated_statuses(&mut services, &target, &others);
    Ok(to_info(&target))
}

#[tauri::command]
pub async fn start_all(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    let mut working = { state.services.read().await.clone() };
    let config = state.config.read().await;
    let active_web = config.web_server.active.clone();
    drop(config);

    push_log(
        &state,
        LogLevel::Info,
        "service",
        format!("全部启动：开始（活跃 web = {}）", active_web),
        None,
        None,
    )
    .await;

    let mut errors = Vec::new();

    for idx in 0..working.len() {
        let kind = working[idx].kind;
        match kind {
            ServiceKind::Nginx if active_web != "nginx" => continue,
            ServiceKind::Apache if active_web != "apache" => continue,
            _ => {}
        }

        // 同 kind 多版本（除 PHP 外）只启动一个：之前迭代里同类已经跑起来就跳过本版本，
        // 避免端口冲突导致 exit code 1 的迷惑失败日志。PHP 多端口可共存，不限制。
        if kind != ServiceKind::Php {
            let target_id = working[idx].id();
            let same_kind_running = working
                .iter()
                .any(|s| s.kind == kind && s.id() != target_id && s.status.is_running());
            if same_kind_running {
                tracing::info!(
                    service = working[idx].kind.display_name(),
                    version = %working[idx].version,
                    "全部启动跳过：同类其他版本已在运行"
                );
                continue;
            }
        }

        let mut target = working[idx].clone();
        let mut others: Vec<_> = working
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != idx)
            .map(|(_, s)| s.clone())
            .collect();

        let result = if matches!(target.kind, ServiceKind::Nginx | ServiceKind::Apache) {
            state
                .service_manager
                .start_with_deps(&mut target, &mut others)
                .await
        } else {
            state.service_manager.start_service(&mut target).await
        };

        if let Err(e) = result {
            let msg = format!("{} {} 启动失败: {}", target.kind.display_name(), target.version, e);
            errors.push(msg.clone());
            push_log(
                &state,
                LogLevel::Error,
                "service",
                format!("{} {} 启动失败", target.kind.display_name(), target.version),
                Some(e.to_string()),
                None,
            )
            .await;
        }

        sync_updated_statuses(&mut working, &target, &others);
    }

    {
        let mut services = state.services.write().await;
        sync_all_statuses(&mut services, &working);
    }

    if errors.is_empty() {
        push_log(
            &state,
            LogLevel::Success,
            "service",
            "全部启动：完成",
            None,
            None,
        )
        .await;
    } else {
        push_log(
            &state,
            LogLevel::Warn,
            "service",
            format!("全部启动：完成（{} 个失败）", errors.len()),
            Some(errors.join("\n")),
            None,
        )
        .await;
    }

    let services = state.services.read().await;
    let infos = all_infos(&services);
    drop(services);

    if errors.is_empty() {
        Ok(infos)
    } else {
        Err(errors.join("\n"))
    }
}

/// 给前端按钮（如手动刷新）写一条日志的小工具命令。前端只能通过命令触发后端 push_log。
#[tauri::command]
pub async fn log_user_action(
    message: String,
    details: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    push_log(&state, LogLevel::Info, "user", message, details, None).await;
    Ok(())
}

#[tauri::command]
pub async fn stop_all(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    let working = { state.services.read().await.clone() };

    push_log(
        &state,
        LogLevel::Info,
        "service",
        format!("全部停止：开始（{} 个服务）", working.len()),
        None,
        None,
    )
    .await;

    // 并行停止：每个 stop_service 内部会优雅停 + 等端口释放，最长 ~3s。
    // 串行下 N 个服务就要 N×3s，前端 10s 超时直接撞墙；服务互相独立可并行。
    let mgr = state.service_manager.clone();
    let futures = working.into_iter().map(|mut svc| {
        let mgr = mgr.clone();
        async move {
            let res = mgr.stop_service(&mut svc).await;
            (svc, res)
        }
    });
    let results = futures_util::future::join_all(futures).await;

    let mut working: Vec<ServiceInstance> = Vec::with_capacity(results.len());
    let mut errors = Vec::new();
    for (svc, res) in results {
        if let Err(e) = res {
            let msg = format!("{} {} 停止失败: {}", svc.kind.display_name(), svc.version, e);
            errors.push(msg.clone());
            push_log(
                &state,
                LogLevel::Error,
                "service",
                format!("{} {} 停止失败", svc.kind.display_name(), svc.version),
                Some(e.to_string()),
                None,
            )
            .await;
        }
        working.push(svc);
    }

    {
        let mut services = state.services.write().await;
        sync_all_statuses(&mut services, &working);
    }

    if errors.is_empty() {
        push_log(
            &state,
            LogLevel::Success,
            "service",
            "全部停止：完成",
            None,
            None,
        )
        .await;
    } else {
        push_log(
            &state,
            LogLevel::Warn,
            "service",
            format!("全部停止：完成（{} 个失败）", errors.len()),
            Some(errors.join("\n")),
            None,
        )
        .await;
    }

    let services = state.services.read().await;
    let infos = all_infos(&services);
    drop(services);

    if errors.is_empty() {
        Ok(infos)
    } else {
        Err(errors.join("\n"))
    }
}
