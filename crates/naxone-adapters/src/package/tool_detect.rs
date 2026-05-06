//! 检测系统里**已经装好**的工具类（composer / nvm-windows）。
//!
//! 用途：商店安装前先看看用户系统里是不是已经有了。如果有，UI 标"系统已装"，
//! 禁用商店的安装/卸载按钮，避免 NaxOne 覆盖用户原有环境（特别是 nvm 的 NVM_HOME）。
//!
//! 检测策略：
//!
//! - **composer**：在用户 PATH 找 `composer.bat` / `composer.phar` / `composer`，
//!   跑 `--version`。排除 NaxOne 自己装的（路径在 `<packages_root>/tools/composer-*` 下）
//! - **nvm-windows**：读环境变量 `NVM_HOME`（系统级或用户级都查），里面要有 `nvm.exe`，
//!   跑 `version`。同样排除 NaxOne 自己装的
//!
//! 失败按"未装"返回 `None` —— 命令不在 / 跑出错都不算装上。

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize)]
pub struct DetectedTool {
    pub name: String,
    pub version: String,
    pub install_path: String,
}

/// 入口：检测一个工具名（"composer" / "nvm"）的系统级安装。
/// `packages_root` 用来排除 NaxOne 自己装的（避免重复计入）。
pub fn detect(name: &str, packages_root: &Path) -> Option<DetectedTool> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (name, packages_root);
        return None;
    }
    #[cfg(target_os = "windows")]
    match name {
        "composer" => detect_composer(packages_root),
        "nvm" => detect_nvm(packages_root),
        _ => None,
    }
}

/// 一次性扫所有支持的工具。
pub fn detect_all(packages_root: &Path) -> Vec<DetectedTool> {
    let mut out = Vec::new();
    for name in ["composer", "nvm"] {
        if let Some(t) = detect(name, packages_root) {
            out.push(t);
        }
    }
    out
}

// ──────────────────────────────────────────────────────────────────────────
// composer
// ──────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn detect_composer(packages_root: &Path) -> Option<DetectedTool> {
    let naxone_tools = packages_root.join("tools");

    let candidates = which_all("composer");
    for cand in &candidates {
        if cand.starts_with(&naxone_tools) {
            continue;
        }

        let dir = match cand.parent() {
            Some(d) => d,
            None => continue,
        };
        let install_path = dir.display().to_string();

        // 尝试 1：直接运行（需要 php 在 PATH 里）
        if let Some(version) = run_version(cand, &["--version", "--no-ansi"], parse_composer_version) {
            return Some(DetectedTool { name: "composer".into(), version, install_path });
        }

        // 运行失败（多半是 php 不在 PATH）。找同目录的 composer.phar 做 fallback。
        let phar = dir.join("composer.phar");
        if !phar.is_file() {
            continue;
        }

        // 尝试 2：用 NaxOne 的全局 PHP shim 跑 phar
        if let Some(version) = try_phar_with_naxone_php(&phar) {
            return Some(DetectedTool { name: "composer".into(), version, install_path });
        }

        // 尝试 3：从 phar 二进制里 grep 版本号
        if let Some(version) = extract_phar_version(&phar) {
            return Some(DetectedTool { name: "composer".into(), version, install_path });
        }

        // 文件确实存在，只是版本拿不到
        return Some(DetectedTool { name: "composer".into(), version: "?".into(), install_path });
    }
    None
}

#[cfg(target_os = "windows")]
fn try_phar_with_naxone_php(phar: &Path) -> Option<String> {
    let home = std::env::var("USERPROFILE").ok()?;
    let php_cmd = PathBuf::from(home).join(".naxone").join("bin").join("php.cmd");
    if !php_cmd.is_file() {
        return None;
    }
    run_version(&php_cmd, &[phar.to_str()?, "--version", "--no-ansi"], parse_composer_version)
}

#[cfg(target_os = "windows")]
fn extract_phar_version(phar: &Path) -> Option<String> {
    let data = std::fs::read(phar).ok()?;
    let text = String::from_utf8_lossy(&data);
    for chunk in text.as_ref().split("Composer version ").skip(1) {
        if let Some(ver) = chunk.split_whitespace().next() {
            if ver.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return Some(ver.to_string());
            }
        }
    }
    None
}

/// `Composer version 2.7.7 2024-...` → `2.7.7`
fn parse_composer_version(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        if let Some(rest) = line.trim().strip_prefix("Composer version ") {
            if let Some(ver) = rest.split_whitespace().next() {
                return Some(ver.to_string());
            }
        }
    }
    None
}

// ──────────────────────────────────────────────────────────────────────────
// nvm-windows
// ──────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn detect_nvm(packages_root: &Path) -> Option<DetectedTool> {
    let nvm_home = read_env_var_user_or_system("NVM_HOME")?;
    let nvm_home = PathBuf::from(nvm_home);
    if nvm_home.as_os_str().is_empty() {
        return None;
    }

    // 排除 NaxOne 自己装的
    let naxone_tools = packages_root.join("tools");
    if nvm_home.starts_with(&naxone_tools) {
        return None;
    }

    let nvm_exe = nvm_home.join("nvm.exe");
    if !nvm_exe.exists() {
        return None;
    }

    let version = run_version(&nvm_exe, &["version"], parse_nvm_version)?;
    Some(DetectedTool {
        name: "nvm".into(),
        version,
        install_path: nvm_home.display().to_string(),
    })
}

/// nvm-windows 的 `nvm version` 直接输出版本号，比如 `1.2.2`
fn parse_nvm_version(stdout: &str) -> Option<String> {
    let line = stdout.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    // 可能带前缀 "v"
    let v = line.trim_start_matches('v');
    // 简单校验：必须看起来像版本号
    if v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        Some(v.to_string())
    } else {
        None
    }
}

// ──────────────────────────────────────────────────────────────────────────
// NVM / Node.js 查询（仪表盘用）
// ──────────────────────────────────────────────────────────────────────────

/// 读 NVM_HOME 环境变量（HKCU 优先）。
pub fn get_nvm_home() -> Option<PathBuf> {
    #[cfg(not(target_os = "windows"))]
    return None;
    #[cfg(target_os = "windows")]
    {
        let val = read_env_var_user_or_system("NVM_HOME")?;
        let p = PathBuf::from(&val);
        if p.as_os_str().is_empty() { None } else { Some(p) }
    }
}

/// 扫描 NVM_HOME 下已安装的 Node.js 版本（降序：最新在前）。
pub fn list_node_versions(nvm_home: &Path) -> Vec<String> {
    let mut versions = Vec::new();
    let Ok(entries) = std::fs::read_dir(nvm_home) else { return versions };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('v') && entry.path().join("node.exe").exists() {
            versions.push(name.trim_start_matches('v').to_string());
        }
    }
    versions.sort_by(|a, b| cmp_semver(b, a));
    versions
}

/// 运行 `node --version` 获取当前活动的 Node.js 版本。
pub fn get_current_node_version() -> Option<String> {
    #[cfg(not(target_os = "windows"))]
    return None;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let out = std::process::Command::new("node")
            .arg("--version")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !out.status.success() { return None; }
        let v = String::from_utf8_lossy(&out.stdout);
        let v = v.trim().trim_start_matches('v');
        if v.is_empty() { None } else { Some(v.to_string()) }
    }
}

/// 调用 `nvm use <version>` 切换当前 Node.js。
pub fn switch_node(nvm_exe: &Path, version: &str) -> Result<String, String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (nvm_exe, version);
        return Err("unsupported".into());
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let out = std::process::Command::new(nvm_exe)
            .args(["use", version])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("nvm use 失败: {}", e))?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        if !out.status.success() && stdout.trim().is_empty() {
            return Err(format!("nvm use 失败: {}", stderr.trim()));
        }
        Ok(stdout)
    }
}

fn cmp_semver(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let pb: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
    pa.cmp(&pb)
}

// ──────────────────────────────────────────────────────────────────────────
// MySQL（仪表盘用）
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct MysqlInstall {
    pub version: String,
    pub install_path: String,
    pub data_dir: String,
    pub port: u16,
    pub initialized: bool,
}

/// 扫 packages_root 下所有 MySQL* 目录（带 bin/mysqld.exe 的）。
pub fn list_installed_mysql(packages_root: &Path) -> Vec<MysqlInstall> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(packages_root) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.to_ascii_lowercase().starts_with("mysql") {
            continue;
        }
        if !path.join("bin").join("mysqld.exe").is_file() {
            continue;
        }
        let version = name
            .trim_start_matches(|c: char| !c.is_ascii_digit())
            .to_string();
        let data_dir = path.join("data");
        let initialized = data_dir.join("mysql").exists();
        let port = read_port_from_my_ini(&path.join("my.ini")).unwrap_or(3306);
        out.push(MysqlInstall {
            version,
            install_path: path.display().to_string(),
            data_dir: data_dir.display().to_string(),
            port,
            initialized,
        });
    }
    out.sort_by(|a, b| cmp_semver(&b.version, &a.version));
    out
}

/// 读 .naxone-root.txt（不存在或读失败返回空字符串）
pub fn read_mysql_root_password(install_path: &Path) -> String {
    std::fs::read_to_string(install_path.join(".naxone-root.txt"))
        .map(|s| s.trim_end_matches(['\r', '\n']).to_string())
        .unwrap_or_default()
}

pub fn write_mysql_root_password(install_path: &Path, pwd: &str) -> std::io::Result<()> {
    std::fs::write(install_path.join(".naxone-root.txt"), pwd.as_bytes())
}

/// 通过运行中的 mysqld 改 root 密码。前置：mysqld 必须已启动且 current_pwd 正确。
pub fn change_mysql_root_password(
    install_path: &Path,
    port: u16,
    current_pwd: &str,
    new_pwd: &str,
) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (install_path, port, current_pwd, new_pwd);
        return Err("unsupported".into());
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let mysql_exe = install_path.join("bin").join("mysql.exe");
        if !mysql_exe.is_file() {
            return Err(format!("找不到 {}", mysql_exe.display()));
        }
        // 转义 SQL 单引号
        let escaped = new_pwd.replace('\'', "''");
        let sql = format!(
            "ALTER USER 'root'@'localhost' IDENTIFIED BY '{}'; FLUSH PRIVILEGES;",
            escaped
        );
        let mut cmd = std::process::Command::new(&mysql_exe);
        cmd.args([
            format!("-P{}", port).as_str(),
            "-h127.0.0.1",
            "-uroot",
            format!("-p{}", current_pwd).as_str(),
            "--protocol=TCP",
            "-e",
            sql.as_str(),
        ]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let out = cmd
            .output()
            .map_err(|e| format!("调用 mysql.exe 失败: {}", e))?;
        if out.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // mysql 客户端"Using a password..."的提醒走 stderr，且当前密码错时也会有"Access denied"
            Err(stderr.trim().to_string())
        }
    }
}

/// 简易解析 my.ini 里 [mysqld] 段的 port=
fn read_port_from_my_ini(ini: &Path) -> Option<u16> {
    let content = std::fs::read_to_string(ini).ok()?;
    let mut in_mysqld = false;
    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_mysqld = line.eq_ignore_ascii_case("[mysqld]");
            continue;
        }
        if !in_mysqld {
            continue;
        }
        if let Some(rest) = line.strip_prefix("port") {
            // port=3306  /  port = 3306
            let v = rest.trim_start_matches(|c: char| c == '=' || c.is_whitespace());
            if let Ok(p) = v.trim().parse() {
                return Some(p);
            }
        }
    }
    None
}

// ──────────────────────────────────────────────────────────────────────────
// 内部工具
// ──────────────────────────────────────────────────────────────────────────

/// 跑 `<exe> <args...>`，把 stdout 给 parser。失败/parser 返回 None 就 None。
#[cfg(target_os = "windows")]
fn run_version<F>(exe: &Path, args: &[&str], parse: F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let out = std::process::Command::new(exe)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !out.status.success() && out.stdout.is_empty() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    parse(&stdout)
}

/// 读用户级或系统级环境变量。优先用户级（用户自己配的 nvm 一般在 HKCU）。
#[cfg(target_os = "windows")]
fn read_env_var_user_or_system(name: &str) -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    if let Ok(k) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("Environment", KEY_READ)
    {
        if let Ok(v) = k.get_value::<String, _>(name) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    let hklm_path = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";
    if let Ok(k) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(hklm_path, KEY_READ) {
        if let Ok(v) = k.get_value::<String, _>(name) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

/// 在 PATH 里找所有匹配的可执行文件（含 PATHEXT 扩展）。
/// 类似 PowerShell `Get-Command -All`，返回所有命中。
#[cfg(target_os = "windows")]
fn which_all(name: &str) -> Vec<PathBuf> {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
    let exts: Vec<String> = pathext
        .split(';')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let mut out = Vec::new();
    for dir in std::env::split_paths(&path_var) {
        // 既试 name 本身（带扩展名的情况，如 "composer.bat"），也试每个 PATHEXT 扩展
        let candidates: Vec<PathBuf> = std::iter::once(dir.join(name))
            .chain(exts.iter().map(|ext| dir.join(format!("{}{}", name, ext))))
            .collect();
        for cand in candidates {
            if cand.is_file() {
                // 去重（同一文件可能由 name + ext 两条路径都命中）
                let canon = std::fs::canonicalize(&cand).unwrap_or_else(|_| cand.clone());
                if !out.iter().any(|p: &PathBuf| {
                    std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()) == canon
                }) {
                    out.push(cand);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_composer_version() {
        let out = "Composer version 2.7.7 2024-06-10 22:11:12\n";
        assert_eq!(parse_composer_version(out), Some("2.7.7".into()));

        let out2 = "Composer version 2.9.7 2025-01-01 10:00:00\nPHP version 8.4.0\n";
        assert_eq!(parse_composer_version(out2), Some("2.9.7".into()));

        assert_eq!(parse_composer_version("garbage\n"), None);
    }

    #[test]
    fn parses_nvm_version() {
        assert_eq!(parse_nvm_version("1.2.2\n"), Some("1.2.2".into()));
        assert_eq!(parse_nvm_version("v1.1.12\r\n"), Some("1.1.12".into()));
        assert_eq!(parse_nvm_version(""), None);
        assert_eq!(parse_nvm_version("not-a-version\n"), None);
    }
}
