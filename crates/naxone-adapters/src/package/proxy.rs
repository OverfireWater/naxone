//! 探测要用的出站代理。优先级：
//! 1. 环境变量 HTTPS_PROXY / HTTP_PROXY（reqwest 本身就会读，这里只用于日志）
//! 2. Windows 系统代理（IE / Edge 设置里的 ProxyServer，Clash/V2ray "系统代理"
//!    开关会写到这里）
//!
//! 返回值示例："http://127.0.0.1:7890"。调用方可以丢给 `reqwest::Proxy::all()`。

/// 返回当前应使用的代理 URL。
pub fn detect_proxy() -> Option<String> {
    // reqwest 自己会读 HTTPS_PROXY/HTTP_PROXY，让它自动生效就够了；我们只需要在
    // 环境变量没设时补上"系统代理"这条兜底。
    if std::env::var("HTTPS_PROXY").ok().filter(|s| !s.is_empty()).is_some()
        || std::env::var("https_proxy").ok().filter(|s| !s.is_empty()).is_some()
    {
        return None; // 让 reqwest 走环境变量
    }
    detect_system_proxy()
}

#[cfg(windows)]
fn detect_system_proxy() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
        .ok()?;

    let enabled: u32 = settings.get_value("ProxyEnable").ok()?;
    if enabled == 0 {
        return None;
    }

    let server: String = settings.get_value("ProxyServer").ok()?;
    // server 可能是：
    //   "127.0.0.1:7890"                            ← 所有协议同一个
    //   "http=127.0.0.1:7890;https=127.0.0.1:7891"  ← 按协议分别指定
    let raw = pick_https_server(&server)?;
    // 补上 scheme，方便 reqwest::Proxy::all 接受
    if raw.starts_with("http://") || raw.starts_with("https://") {
        Some(raw)
    } else {
        Some(format!("http://{}", raw))
    }
}

#[cfg(not(windows))]
fn detect_system_proxy() -> Option<String> {
    None
}

/// 从 `"http=...;https=...;ftp=..."` 或 `"host:port"` 这两种格式里挑 HTTPS 代理。
fn pick_https_server(s: &str) -> Option<String> {
    if !s.contains('=') {
        // 简单格式：所有协议共用
        return Some(s.trim().to_string());
    }
    // 分段格式：找 https= 优先，否则退回 http=
    let mut https_val = None;
    let mut http_val = None;
    for part in s.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            match k.trim().to_lowercase().as_str() {
                "https" => https_val = Some(v.trim().to_string()),
                "http" => http_val = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    https_val.or(http_val)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_host_port() {
        assert_eq!(pick_https_server("127.0.0.1:7890"), Some("127.0.0.1:7890".into()));
    }

    #[test]
    fn per_protocol_prefers_https() {
        let s = "http=a:1;https=b:2;ftp=c:3";
        assert_eq!(pick_https_server(s), Some("b:2".into()));
    }

    #[test]
    fn per_protocol_falls_back_to_http() {
        let s = "http=a:1;ftp=c:3";
        assert_eq!(pick_https_server(s), Some("a:1".into()));
    }

    #[test]
    fn per_protocol_none_matches() {
        assert_eq!(pick_https_server("ftp=c:3"), None);
    }
}
