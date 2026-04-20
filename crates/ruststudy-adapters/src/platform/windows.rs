use std::path::{Path, PathBuf};

use ruststudy_core::error::{Result, RustStudyError};
use ruststudy_core::ports::platform::PlatformOps;

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// 读取 hosts 文件：容忍 UTF-8 BOM 与非 UTF-8 字节（lossy）
/// 返回 (text_without_bom, had_bom)
fn read_hosts(path: &Path) -> Result<(String, bool)> {
    let bytes = std::fs::read(path)
        .map_err(|e| RustStudyError::from_io_with_context(e, "读取 hosts 文件失败"))?;
    let (slice, had_bom) = if bytes.starts_with(UTF8_BOM) {
        (&bytes[3..], true)
    } else {
        (&bytes[..], false)
    };
    // 非 UTF-8 字节替换为 U+FFFD，不让一处坏字符毁掉整个文件
    let text = String::from_utf8_lossy(slice).into_owned();
    Ok((text, had_bom))
}

/// 写回 hosts：按原有 BOM 情况保留
fn write_hosts(path: &Path, content: &str, had_bom: bool) -> Result<()> {
    let mut out: Vec<u8> = Vec::with_capacity(content.len() + 3);
    if had_bom {
        out.extend_from_slice(UTF8_BOM);
    }
    out.extend_from_slice(content.as_bytes());
    std::fs::write(path, out)
        .map_err(|e| RustStudyError::from_io_with_context(e, "写入 hosts 文件失败"))
}

pub struct WindowsPlatform;

impl PlatformOps for WindowsPlatform {
    fn hosts_file_path(&self) -> PathBuf {
        PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts")
    }

    fn add_hosts_entry(&self, hostname: &str, ip: &str) -> Result<()> {
        let hosts_path = self.hosts_file_path();
        let (content, had_bom) = read_hosts(&hosts_path)?;

        let entry = format!("{} {}", ip, hostname);
        // 已存在完整条目则跳过
        if content.lines().any(|l| {
            let t = l.trim();
            !t.starts_with('#') && t == entry
        }) {
            return Ok(());
        }

        let new_content = format!("{}\n{}\n", content.trim_end(), entry);
        write_hosts(&hosts_path, &new_content, had_bom)
    }

    fn remove_hosts_entry(&self, hostname: &str) -> Result<()> {
        let hosts_path = self.hosts_file_path();
        let (content, had_bom) = read_hosts(&hosts_path)?;

        let filtered: Vec<&str> = content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    return true;
                }
                // 按空白分割后，域名出现在第二段开始（ip hostname [alias...]）
                let mut parts = trimmed.split_whitespace();
                let _ip = parts.next();
                !parts.any(|p| p == hostname)
            })
            .collect();

        write_hosts(&hosts_path, &filtered.join("\n"), had_bom)
    }

    fn data_dir(&self) -> PathBuf {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
        PathBuf::from(home).join(".ruststudy")
    }

    fn add_firewall_port(&self, port: u16) -> Result<()> {
        let name = firewall_rule_name(port);
        // 幂等 upsert：先删同名规则（忽略结果），再添加新规则
        let _ = netsh_delete_rule(&name);
        netsh_add_rule(&name, port)
    }

    fn remove_firewall_port(&self, port: u16) -> Result<()> {
        netsh_delete_rule(&firewall_rule_name(port))
    }
}

fn firewall_rule_name(port: u16) -> String {
    format!("RustStudy port {}", port)
}

/// `netsh advfirewall firewall add rule name=... dir=in action=allow protocol=TCP localport=...`
fn netsh_add_rule(name: &str, port: u16) -> Result<()> {
    let out = std::process::Command::new("netsh")
        .args([
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={}", name),
            "dir=in",
            "action=allow",
            "protocol=TCP",
            &format!("localport={}", port),
        ])
        .output()
        .map_err(|e| RustStudyError::Process(format!("无法调用 netsh: {}", e)))?;
    if out.status.success() {
        return Ok(());
    }
    Err(classify_netsh_err(&out))
}

/// `netsh advfirewall firewall delete rule name=...`
/// 不存在对应规则时 netsh 也返回非零但文本里会有 "No rules match" —— 这种情况视为 Ok
fn netsh_delete_rule(name: &str) -> Result<()> {
    let out = std::process::Command::new("netsh")
        .args([
            "advfirewall",
            "firewall",
            "delete",
            "rule",
            &format!("name={}", name),
        ])
        .output()
        .map_err(|e| RustStudyError::Process(format!("无法调用 netsh: {}", e)))?;
    if out.status.success() {
        return Ok(());
    }
    let combined = combined_output(&out);
    if combined.contains("No rules match")
        || combined.contains("找不到与指定条件相符的规则")
        || combined.contains("没有与指定条件匹配的规则")
    {
        return Ok(());
    }
    Err(classify_netsh_err(&out))
}

fn combined_output(out: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    format!("{}{}", stdout.trim(), stderr.trim())
}

fn classify_netsh_err(out: &std::process::Output) -> RustStudyError {
    let combined = combined_output(out);
    let lower = combined.to_lowercase();
    if lower.contains("elevation")
        || lower.contains("access is denied")
        || combined.contains("管理员")
        || combined.contains("拒绝访问")
        || combined.contains("需要")
    {
        RustStudyError::PermissionDenied(format!(
            "修改防火墙需要以管理员身份运行 RustStudy: {}",
            combined
        ))
    } else {
        RustStudyError::Process(format!("netsh 执行失败: {}", combined))
    }
}
