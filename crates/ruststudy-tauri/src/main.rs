#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use state::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

fn main() {
    // 默认 info 级别，方便排查安装/代理/状态刷新相关的诊断日志
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,hyper=warn,reqwest=warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let app_state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::service::get_services,
            commands::service::get_services_fresh,
            commands::service::start_service,
            commands::service::stop_service,
            commands::service::restart_service,
            commands::service::start_all,
            commands::service::stop_all,
            commands::vhost::get_vhosts,
            commands::vhost::create_vhost,
            commands::vhost::update_vhost,
            commands::vhost::delete_vhost,
            commands::vhost::toggle_vhost,
            commands::vhost::check_expired_vhosts,
            commands::vhost::get_php_versions,
            commands::vhost::generate_self_signed_cert,
            commands::php::get_php_instances,
            commands::php::get_php_extensions,
            commands::php::toggle_php_extension,
            commands::php::get_php_ini_settings,
            commands::php::save_php_ini_settings,
            commands::php::get_global_php_version,
            commands::php::set_global_php_version,
            commands::php::fix_global_php_conflicts,
            commands::php::open_system_env_editor,
            commands::settings::get_config,
            commands::settings::save_config,
            commands::settings::rescan_services,
            commands::settings::add_extra_install_path,
            commands::settings::remove_extra_install_path,
            commands::settings::check_phpstudy_installed,
            commands::service_config::get_config_file_path,
            commands::service_config::get_nginx_config,
            commands::service_config::save_nginx_config,
            commands::service_config::get_mysql_config,
            commands::service_config::save_mysql_config,
            commands::service_config::get_redis_config,
            commands::service_config::save_redis_config,
            commands::utils::get_app_stats,
            commands::utils::open_in_browser,
            commands::utils::open_folder,
            commands::utils::open_file,
            commands::utils::check_port_available,
            commands::utils::dir_exists,
            commands::utils::read_log_tail,
            commands::utils::find_and_read_log,
            commands::utils::get_startup_errors,
            commands::updater::check_for_updates,
            commands::logger::get_logs,
            commands::logger::clear_logs,
            commands::logger::get_log_stats,
            commands::logger::open_log_dir,
            commands::package::list_packages,
            commands::package::refresh_package_index,
            commands::package::get_installed_packages,
            commands::package::install_package,
            commands::package::uninstall_package,
        ])
        .setup(|app| {
            // Start log writer background task
            {
                let state = app.state::<AppState>();
                let state_arc: std::sync::Arc<AppState> = std::sync::Arc::new(state.inner().clone_shallow());
                let tx = commands::logger::spawn_log_writer(state_arc);
                // Blocking set the sender - we're in setup, before runtime is fully started
                let tx_slot = state.log_writer_tx.clone();
                tauri::async_runtime::block_on(async move {
                    *tx_slot.lock().await = Some(tx);
                });
            }

            // Clean up old log files
            {
                let state = app.state::<AppState>();
                let config = state.config.clone();
                tauri::async_runtime::spawn(async move {
                    let cfg = config.read().await.clone();
                    let dir = state::resolve_log_dir(&cfg);
                    commands::logger::cleanup_old_logs(&dir, cfg.general.log_retention_days);
                });
            }

            // Log startup (with a concise scan summary — useful for debugging
            // "no services showing" reports without flooding the log)
            {
                let state = app.state::<AppState>();
                let state_clone: std::sync::Arc<AppState> = std::sync::Arc::new((*state.inner()).clone_shallow());
                tauri::async_runtime::spawn(async move {
                    let svc_count = state_clone.services.read().await.len();
                    commands::logger::push_log(
                        &state_clone,
                        ruststudy_core::domain::log::LogLevel::Info,
                        "system",
                        format!("RustStudy 启动（扫到 {} 个服务）", svc_count),
                        None, None,
                    ).await;
                });
            }

            // 启动预热：并行刷新一次所有服务 status，让第一次 get_services 直接命中
            {
                let state = app.state::<AppState>();
                let warmup_state = state.inner().clone_shallow();
                tauri::async_runtime::spawn(async move {
                    let t = std::time::Instant::now();
                    commands::service::refresh_all_services_bg(warmup_state).await;
                    tracing::info!(elapsed_ms = t.elapsed().as_millis() as u64, "预热 status 缓存完成");
                });
            }

            // Auto-start services based on config
            {
                let state = app.state::<AppState>();
                let services = state.services.clone();
                let config = state.config.clone();
                let service_manager = state.service_manager.clone();
                let errors = state.startup_errors.clone();
                tauri::async_runtime::spawn(async move {
                    let auto_start = config.read().await.general.auto_start.clone();
                    if auto_start.is_empty() { return; }
                    let mut svcs = services.write().await;
                    for svc in svcs.iter_mut() {
                        let kind_name = svc.kind.display_name().to_lowercase();
                        if !auto_start.iter().any(|a| kind_name.contains(a)) {
                            continue;
                        }
                        // 先刷一次状态：初始 status=Stopped 是占位，端口已被占
                        // （比如 PHPStudy 自己启动的实例）时应跳过而非重复启动
                        let _ = service_manager.refresh_status(svc).await;
                        if svc.status.is_running() {
                            tracing::info!(
                                service = svc.kind.display_name(),
                                "自动启动跳过：已在运行"
                            );
                            continue;
                        }
                        if let Err(e) = service_manager.start_service(svc).await {
                            let msg = format!("{} 自动启动失败: {}", svc.kind.display_name(), e);
                            tracing::error!("{}", &msg);
                            errors.write().await.push(msg);
                        }
                    }
                });
            }

            // Build tray menu
            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            // Create tray icon
            let icon = app.default_window_icon().cloned()
                .expect("No default window icon");

            TrayIconBuilder::new()
                .icon(icon)
                .tooltip("RustStudy")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // Minimize to tray on close (instead of quitting)
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running RustStudy");
}
