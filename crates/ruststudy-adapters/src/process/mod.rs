#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

// Re-export the platform-appropriate process manager
#[cfg(target_os = "windows")]
pub use windows::WindowsProcessManager as NativeProcessManager;

#[cfg(target_os = "linux")]
pub use linux::LinuxProcessManager as NativeProcessManager;
