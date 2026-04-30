use std::path::PathBuf;
use tauri::State;

use crate::commands::logger::push_log;
use crate::state::AppState;
use naxone_core::domain::log::LogLevel;
use naxone_core::domain::service::ServiceKind;
use naxone_core::use_cases::config_editor::{MysqlConfig, NginxConfig, RedisConfig};

fn find_service_config_path(
    _state: &AppState,
    services: &[naxone_core::domain::service::ServiceInstance],
    kind: ServiceKind,
) -> Option<PathBuf> {
    services
        .iter()
        .find(|s| s.kind == kind)
        .and_then(|s| {
            s.config_path.clone().or_else(|| {
                match kind {
                    ServiceKind::Nginx => Some(s.install_path.join("conf").join("nginx.conf")),
                    ServiceKind::Mysql => Some(s.install_path.join("my.ini")),
                    ServiceKind::Redis => Some(s.install_path.join("redis.windows.conf")),
                    _ => None,
                }
            })
        })
}

/// Get the config file path for a service
#[tauri::command]
pub async fn get_config_file_path(service: String, state: State<'_, AppState>) -> Result<String, String> {
    let services = state.services.read().await;
    let kind = match service.as_str() {
        "nginx" => ServiceKind::Nginx,
        "mysql" => ServiceKind::Mysql,
        "redis" => ServiceKind::Redis,
        _ => return Err(format!("Unknown service: {}", service)),
    };
    let path = find_service_config_path(&state, &services, kind)
        .ok_or(format!("{} config not found", service))?;
    Ok(path.display().to_string())
}

// ======================== Nginx ========================

#[tauri::command]
pub async fn get_nginx_config(state: State<'_, AppState>) -> Result<NginxConfig, String> {
    let services = state.services.read().await;
    let path = find_service_config_path(&state, &services, ServiceKind::Nginx)
        .ok_or("Nginx not found")?;
    state.config_editor.read_nginx(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_nginx_config(cfg: NginxConfig, state: State<'_, AppState>) -> Result<(), String> {
    let services = state.services.read().await;
    let path = find_service_config_path(&state, &services, ServiceKind::Nginx)
        .ok_or("Nginx not found")?;
    state.config_editor.save_nginx(&path, &cfg).map_err(|e| e.to_string())?;
    push_log(&state, LogLevel::Success, "config", "保存 Nginx 配置", None, None).await;
    Ok(())
}

// ======================== MySQL ========================

#[tauri::command]
pub async fn get_mysql_config(state: State<'_, AppState>) -> Result<MysqlConfig, String> {
    let services = state.services.read().await;
    let path = find_service_config_path(&state, &services, ServiceKind::Mysql)
        .ok_or("MySQL not found")?;
    state.config_editor.read_mysql(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_mysql_config(cfg: MysqlConfig, state: State<'_, AppState>) -> Result<(), String> {
    let services = state.services.read().await;
    let path = find_service_config_path(&state, &services, ServiceKind::Mysql)
        .ok_or("MySQL not found")?;
    state.config_editor.save_mysql(&path, &cfg).map_err(|e| e.to_string())?;
    push_log(&state, LogLevel::Success, "config", "保存 MySQL 配置", None, None).await;
    Ok(())
}

// ======================== Redis ========================

#[tauri::command]
pub async fn get_redis_config(state: State<'_, AppState>) -> Result<RedisConfig, String> {
    let services = state.services.read().await;
    let path = find_service_config_path(&state, &services, ServiceKind::Redis)
        .ok_or("Redis not found")?;
    state.config_editor.read_redis(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_redis_config(cfg: RedisConfig, state: State<'_, AppState>) -> Result<(), String> {
    let services = state.services.read().await;
    let path = find_service_config_path(&state, &services, ServiceKind::Redis)
        .ok_or("Redis not found")?;
    state.config_editor.save_redis(&path, &cfg).map_err(|e| e.to_string())?;
    push_log(&state, LogLevel::Success, "config", "保存 Redis 配置", None, None).await;
    Ok(())
}
