#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod global_php;

#[cfg(target_os = "windows")]
pub mod user_env;

pub mod ssl_cert;

pub mod dirs;
