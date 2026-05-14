use std::path::{Path, PathBuf};

use naxone_core::error::{Result, NaxOneError};
use naxone_core::ports::platform::PlatformOps;

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// 读取 hosts 文件：容忍 UTF-8 BOM 与非 UTF-8 字节（lossy）
/// 返回 (text_without_bom, had_bom)
fn read_hosts(path: &Path) -> Result<(String, bool)> {
    let bytes = std::fs::read(path)
        .map_err(|e| NaxOneError::from_io_with_context(e, "读取 hosts 文件失败"))?;
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
        .map_err(|e| NaxOneError::from_io_with_context(e, "写入 hosts 文件失败"))
}

/// UAC 提权写 hosts：先把目标内容写到 temp，再用 `Start-Process -Verb RunAs` 让
/// 提权 PowerShell 把 temp 拷贝到真实 hosts 路径。整个过程只弹一次 UAC。
fn write_hosts_elevated(path: &Path, content: &str, had_bom: bool) -> Result<()> {
    let temp_dir = std::env::temp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_path = temp_dir.join(format!("naxone-hosts-{}-{}.txt", std::process::id(), stamp));
    let script_path = temp_dir.join(format!("naxone-hosts-{}-{}.ps1", std::process::id(), stamp));

    // 把含 BOM 的字节直接写到 temp，提权脚本里 Copy-Item 字节级复制即可保留。
    let mut bytes: Vec<u8> = Vec::with_capacity(content.len() + 3);
    if had_bom {
        bytes.extend_from_slice(UTF8_BOM);
    }
    bytes.extend_from_slice(content.as_bytes());
    std::fs::write(&temp_path, &bytes)
        .map_err(|e| NaxOneError::from_io_with_context(e, "写入临时 hosts 文件失败"))?;

    let host_path_str = path.display().to_string().replace('"', "``\"");
    let src_path_str = temp_path.display().to_string().replace('"', "``\"");
    let script = format!(
        "$ErrorActionPreference = 'Stop'\nCopy-Item -LiteralPath \"{}\" -Destination \"{}\" -Force\n",
        src_path_str, host_path_str
    );
    std::fs::write(&script_path, script.as_bytes())
        .map_err(|e| NaxOneError::from_io_with_context(e, "写入提权脚本失败"))?;

    let launcher = format!(
        "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','{}'",
        script_path.display().to_string().replace('"', "``\"")
    );

    let output = {
        let mut cmd = std::process::Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &launcher,
        ]);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        cmd.output()
            .map_err(|e| NaxOneError::Process(format!("调用提权 PowerShell 失败: {}", e)))?
    };

    let _ = std::fs::remove_file(&temp_path);
    let _ = std::fs::remove_file(&script_path);

    if !output.status.success() {
        let combined = combined_output(&output);
        let lower = combined.to_lowercase();
        if combined.contains("取消") || lower.contains("cancel") || lower.contains("declined") {
            return Err(NaxOneError::PermissionDenied(
                "已取消（未确认管理员权限）".to_string(),
            ));
        }
        return Err(NaxOneError::PermissionDenied(format!(
            "提权写 hosts 失败: {}",
            combined
        )));
    }

    Ok(())
}

pub struct WindowsPlatform;

impl PlatformOps for WindowsPlatform {
    fn hosts_file_path(&self) -> PathBuf {
        PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts")
    }

    fn apply_hosts_changes(
        &self,
        additions: &[(String, String)],
        removals: &[String],
    ) -> Result<()> {
        if additions.is_empty() && removals.is_empty() {
            return Ok(());
        }

        let hosts_path = self.hosts_file_path();
        let (content, had_bom) = read_hosts(&hosts_path)?;

        // 先按 removals 过滤掉相关行，再追加 additions（去重）。
        let mut lines: Vec<String> = content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with('#') || trimmed.is_empty() {
                    return true;
                }
                // ip hostname [alias...] —— 跳过 ip，看其余字段是否命中 removals
                let mut parts = trimmed.split_whitespace();
                let _ip = parts.next();
                !parts.any(|p| removals.iter().any(|r| r == p))
            })
            .map(|s| s.to_string())
            .collect();

        for (hostname, ip) in additions {
            let entry = format!("{} {}", ip, hostname);
            let exists = lines.iter().any(|l| {
                let t = l.trim();
                !t.starts_with('#') && t == entry
            });
            if !exists {
                lines.push(entry);
            }
        }

        let new_content = format!("{}\n", lines.join("\n").trim_end_matches('\n'));

        // 直接写；遇到权限不足走 UAC 提权一次性写入
        match write_hosts(&hosts_path, &new_content, had_bom) {
            Ok(()) => Ok(()),
            Err(NaxOneError::PermissionDenied(_)) => {
                tracing::info!("hosts 直接写入需要管理员权限，启动 UAC 提权");
                write_hosts_elevated(&hosts_path, &new_content, had_bom)
            }
            Err(e) => Err(e),
        }
    }

    fn data_dir(&self) -> PathBuf {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
        PathBuf::from(home).join(".naxone")
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
    format!("NaxOne port {}", port)
}

/// `netsh advfirewall firewall add rule name=... dir=in action=allow protocol=TCP localport=...`
fn netsh_add_rule(name: &str, port: u16) -> Result<()> {
    let out = {
        let mut cmd = std::process::Command::new("netsh");
        cmd.args([
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={}", name),
            "dir=in",
            "action=allow",
            "protocol=TCP",
            &format!("localport={}", port),
        ]);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        cmd.output()
            .map_err(|e| NaxOneError::Process(format!("无法调用 netsh: {}", e)))?
    };
    if out.status.success() {
        return Ok(());
    }
    Err(classify_netsh_err(&out))
}

/// `netsh advfirewall firewall delete rule name=...`
/// 不存在对应规则时 netsh 也返回非零但文本里会有 "No rules match" —— 这种情况视为 Ok
fn netsh_delete_rule(name: &str) -> Result<()> {
    let out = {
        let mut cmd = std::process::Command::new("netsh");
        cmd.args([
            "advfirewall",
            "firewall",
            "delete",
            "rule",
            &format!("name={}", name),
        ]);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        cmd.output()
            .map_err(|e| NaxOneError::Process(format!("无法调用 netsh: {}", e)))?
    };
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

fn classify_netsh_err(out: &std::process::Output) -> NaxOneError {
    let combined = combined_output(out);
    let lower = combined.to_lowercase();
    if lower.contains("elevation")
        || lower.contains("access is denied")
        || combined.contains("管理员")
        || combined.contains("拒绝访问")
        || combined.contains("需要")
    {
        NaxOneError::PermissionDenied(format!(
            "修改防火墙需要以管理员身份运行 NaxOne: {}",
            combined
        ))
    } else {
        NaxOneError::Process(format!("netsh 执行失败: {}", combined))
    }
}
