//! 用户级环境变量与 PATH（HKCU\Environment）操作。
//!
//! 不用 `setx` —— 它在 PATH > 1024 字符时会**静默截断**。直接读写注册表 +
//! 广播 `WM_SETTINGCHANGE`，新开的 cmd / explorer 会立即重读。
//!
//! 这里只动 HKCU，**不需要管理员权限**。系统级（HKLM）改动见 `global_php::fix_masking_paths`。

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::RegKey;

const ENVIRONMENT: &str = "Environment";

/// 把 value 追加到用户 PATH 末尾（已存在则跳过）。
/// 返回 `true` = 实际有变更并已广播；`false` = 已存在。
pub fn append_to_user_path(value: &str) -> std::io::Result<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu.open_subkey_with_flags(ENVIRONMENT, KEY_READ | KEY_SET_VALUE)?;
    let current: String = env_key.get_value("Path").unwrap_or_default();
    if path_contains(&current, value) {
        return Ok(false);
    }
    let new_path = if current.trim_end_matches(';').is_empty() {
        value.to_string()
    } else {
        format!("{};{}", current.trim_end_matches(';'), value)
    };
    env_key.set_value("Path", &new_path)?;
    broadcast_env_change();
    Ok(true)
}

/// 从用户 PATH 移除 value（卸载用）。返回 `true` = 实际有移除。
pub fn remove_from_user_path(value: &str) -> std::io::Result<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu.open_subkey_with_flags(ENVIRONMENT, KEY_READ | KEY_SET_VALUE)?;
    let current: String = env_key.get_value("Path").unwrap_or_default();
    if !path_contains(&current, value) {
        return Ok(false);
    }
    let target = normalize_path(value);
    let kept: Vec<&str> = current
        .split(';')
        .filter(|p| !p.trim().is_empty() && normalize_path(p) != target)
        .collect();
    let new_path = kept.join(";");
    env_key.set_value("Path", &new_path)?;
    broadcast_env_change();
    Ok(true)
}

/// 设置用户级环境变量（HKCU\Environment\<name>）。
/// 已存在且值相等 → 返回 `false`，否则写入并广播，返回 `true`。
pub fn set_user_env_var(name: &str, value: &str) -> std::io::Result<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu.open_subkey_with_flags(ENVIRONMENT, KEY_READ | KEY_SET_VALUE)?;
    let current: String = env_key.get_value(name).unwrap_or_default();
    if current == value {
        return Ok(false);
    }
    env_key.set_value(name, &value.to_string())?;
    broadcast_env_change();
    Ok(true)
}

/// 删除用户级环境变量（不存在则忽略）。
pub fn unset_user_env_var(name: &str) -> std::io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu.open_subkey_with_flags(ENVIRONMENT, KEY_READ | KEY_SET_VALUE)?;
    match env_key.delete_value(name) {
        Ok(()) => {
            broadcast_env_change();
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// 用户 PATH 里是否已包含 target（不区分大小写 + 去尾反斜杠 + / 兼容 \）。
pub fn path_contains(haystack: &str, target: &str) -> bool {
    let t = normalize_path(target);
    haystack.split(';').any(|p| normalize_path(p) == t)
}

/// 读 HKCU\Environment\Path 当前值（读不到返回空串）。
pub fn read_user_path() -> String {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey_with_flags(ENVIRONMENT, KEY_READ)
        .ok()
        .and_then(|k| k.get_value::<String, _>("Path").ok())
        .unwrap_or_default()
}

/// 读 HKLM 系统级 Path（PHPStudy / 安装包通常写这里）。读不到返回空串。
pub fn read_system_path() -> String {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey_with_flags(
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        KEY_READ,
    )
    .ok()
    .and_then(|k| k.get_value::<String, _>("Path").ok())
    .unwrap_or_default()
}

pub fn normalize_path(s: &str) -> String {
    s.trim()
        .trim_end_matches('\\')
        .replace('/', "\\")
        .to_ascii_lowercase()
}

/// 广播 WM_SETTINGCHANGE 到所有顶层窗口，让新 shell 重读环境变量。
/// 已开的 cmd 窗口 OS 限制改不了，必须用户重开。
pub fn broadcast_env_change() {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SendMessageTimeoutW(
            hwnd: isize,
            msg: u32,
            wparam: usize,
            lparam: *const u16,
            flags: u32,
            timeout: u32,
            result: *mut usize,
        ) -> isize;
    }
    const HWND_BROADCAST: isize = 0xffff;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    let msg: Vec<u16> = "Environment\0".encode_utf16().collect();
    let mut result: usize = 0;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            msg.as_ptr(),
            SMTO_ABORTIFHUNG,
            3000,
            &mut result,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_contains_handles_variants() {
        assert!(path_contains(r"C:\a;C:\Users\x\.naxone\bin;C:\b", r"C:\Users\x\.naxone\bin"));
        assert!(path_contains(r"C:\Users\x\.naxone\bin\", r"C:\Users\x\.naxone\bin"));
        assert!(path_contains(r"C:\USERS\X\.NAXONE\BIN", r"c:\users\x\.naxone\bin"));
        assert!(path_contains(r"C:\Users\x/.naxone/bin", r"C:\Users\x\.naxone\bin"));
        assert!(!path_contains(r"C:\a;C:\b", r"C:\Users\x\.naxone\bin"));
        assert!(!path_contains("", r"C:\Users\x\.naxone\bin"));
    }
}
