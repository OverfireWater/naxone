use serde::Serialize;
use tauri::State;

use crate::commands::logger::push_log;
use crate::state::AppState;
use ruststudy_core::domain::log::LogLevel;
use ruststudy_core::domain::php::{PhpExtension, PhpIniSettings};
use ruststudy_core::domain::service::ServiceKind;

#[derive(Debug, Clone, Serialize)]
pub struct PhpInstanceInfo {
    pub id: String,
    pub label: String,
    pub version: String,
    pub variant: Option<String>,
    pub install_path: String,
}

#[tauri::command]
pub async fn get_php_instances(state: State<'_, AppState>) -> Result<Vec<PhpInstanceInfo>, String> {
    let services = state.services.read().await;
    let instances: Vec<PhpInstanceInfo> = services
        .iter()
        .filter(|s| s.kind == ServiceKind::Php)
        .map(|s| PhpInstanceInfo {
            id: s.id(),
            label: format!("PHP {} {}", s.version, s.variant.as_deref().unwrap_or("")),
            version: s.version.clone(),
            variant: s.variant.clone(),
            install_path: s.install_path.display().to_string(),
        })
        .collect();
    Ok(instances)
}

#[tauri::command]
pub async fn get_php_extensions(
    install_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<PhpExtension>, String> {
    state
        .php_manager
        .list_extensions(std::path::Path::new(&install_path))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_php_extension(
    install_path: String,
    ext_name: String,
    enable: bool,
    is_zend: bool,
    state: State<'_, AppState>,
) -> Result<Vec<PhpExtension>, String> {
    state.php_manager.toggle_extension(std::path::Path::new(&install_path), &ext_name, enable, is_zend).map_err(|e| e.to_string())?;
    push_log(&state, LogLevel::Success, "extension",
        if enable { format!("启用扩展 {}", ext_name) } else { format!("禁用扩展 {}", ext_name) },
        None, None).await;

    state.php_manager.list_extensions(std::path::Path::new(&install_path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_php_ini_settings(
    install_path: String,
    state: State<'_, AppState>,
) -> Result<PhpIniSettings, String> {
    state
        .php_manager
        .read_ini_settings(std::path::Path::new(&install_path))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_php_ini_settings(
    install_path: String,
    settings: PhpIniSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.php_manager.save_ini_settings(std::path::Path::new(&install_path), &settings).map_err(|e| e.to_string())?;
    push_log(&state, LogLevel::Success, "config", "保存 PHP 配置", None, None).await;
    Ok(())
}
