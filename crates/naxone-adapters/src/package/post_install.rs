//! 包安装完成后的额外处理（写 shim、改用户 PATH、设环境变量等）。
//!
//! 默认包（PHP/Nginx/MySQL/Redis/Apache）解压即可用，不走这里。
//! 走这里的是**工具类**包：
//!
//! - `composer`: 写 composer.bat 包裹 phar，挂用户 PATH，配阿里云源
//! - `nvm`:      写 settings.txt（含 npmmirror 镜像），设 NVM_HOME / NVM_SYMLINK，挂用户 PATH 两条
//!
//! 全部走 HKCU，不需要管理员。失败按 best-effort 记 warn，不阻塞主安装流程。

use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
use crate::platform::user_env;

/// 主入口。`installer.rs::finalize_install` 在 unzip 完成后调用。
/// 错误只 log，不返回 Err —— 包文件已经落盘可用，post-install 是锦上添花。
pub fn run(name: &str, install_dir: &Path) {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (name, install_dir);
        return;
    }
    #[cfg(target_os = "windows")]
    match name {
        "composer" => composer(install_dir),
        "nvm" => nvm(install_dir),
        "mysql" => mysql(install_dir),
        _ => {}
    }
}

/// 卸载时调用，清理 PATH / 环境变量。
pub fn uninstall(name: &str, install_dir: &Path) {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (name, install_dir);
        return;
    }
    #[cfg(target_os = "windows")]
    match name {
        "composer" => composer_uninstall(install_dir),
        "nvm" => nvm_uninstall(install_dir),
        "mysql" => mysql_uninstall(install_dir),
        _ => {}
    }
}

/// 「解除关联」：仅清环境变量 + PATH 条目，**不删任何文件**。
/// 用户原来的 nvm/composer 文件还在，可以再装回 NaxOne 也可以走系统级。
pub fn unlink(name: &str, install_dir: &Path) -> UnlinkReport {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (name, install_dir);
        return UnlinkReport::default();
    }
    #[cfg(target_os = "windows")]
    match name {
        "composer" => unlink_composer(install_dir),
        "nvm" => unlink_nvm(install_dir),
        _ => UnlinkReport::default(),
    }
}

/// 「彻底卸载」：删工具核心本体文件 + 清环境变量。**保留**用户数据：
/// - composer：保留 `%COMPOSER_HOME%`（global require 装的工具）
/// - nvm：保留 `<NVM_HOME>/v*.*.*` 各 node 版本目录（含全局 npm 包）
pub fn deep_uninstall(name: &str, install_dir: &Path) -> DeepUninstallReport {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (name, install_dir);
        return DeepUninstallReport::default();
    }
    #[cfg(target_os = "windows")]
    match name {
        "composer" => deep_uninstall_composer(install_dir),
        "nvm" => deep_uninstall_nvm(install_dir),
        _ => DeepUninstallReport::default(),
    }
}

/// 仅记录 PATH/env 变化，没文件操作。
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct UnlinkReport {
    /// 被清掉的环境项描述，如 "PATH 条目: C:\..."、"NVM_HOME"
    pub cleared: Vec<String>,
    pub errors: Vec<String>,
}

/// 彻底卸载详情，给前端展示用。
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct DeepUninstallReport {
    pub deleted_files: Vec<String>,
    pub cleared_env: Vec<String>,
    /// 主动保留的用户数据路径（提示用户自己处理）
    pub kept_paths: Vec<String>,
    pub errors: Vec<String>,
}

// ──────────────────────────────────────────────────────────────────────────
// composer
// ──────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn composer(install_dir: &Path) {
    // 1. composer.bat —— 调 NaxOne 全局 php.cmd shim，再传入 phar。
    //    这样切换全局 PHP 后 composer 自动跟着切，不用重新装。
    let php_shim = naxone_php_shim();
    let bat = format!(
        "@ECHO OFF\r\n\"{}\" \"%~dp0composer.phar\" %*\r\n",
        php_shim.display()
    );
    if let Err(e) = std::fs::write(install_dir.join("composer.bat"), bat.as_bytes()) {
        tracing::warn!("写 composer.bat 失败: {}", e);
    }

    // 2. 加用户 PATH
    let install_str = install_dir.display().to_string();
    match user_env::append_to_user_path(&install_str) {
        Ok(true) => tracing::info!("composer 加入用户 PATH: {}", install_str),
        Ok(false) => tracing::debug!("composer PATH 已存在: {}", install_str),
        Err(e) => tracing::warn!("写用户 PATH 失败: {}", e),
    }

    // 3. 配阿里云镜像源（best effort，PHP 没装/没设全局时会失败）
    let bat_path = install_dir.join("composer.bat");
    let result = std::process::Command::new(&bat_path)
        .args([
            "config",
            "-g",
            "repo.packagist",
            "composer",
            "https://mirrors.aliyun.com/composer/",
        ])
        .output();
    match result {
        Ok(o) if o.status.success() => {
            tracing::info!("composer 镜像源已配置为阿里云");
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            tracing::warn!(
                "composer config 配镜像失败（可能 PHP 全局 CLI 未设置）: {}",
                stderr.trim()
            );
        }
        Err(e) => tracing::warn!("composer config 调用失败: {}", e),
    }
}

#[cfg(target_os = "windows")]
fn composer_uninstall(install_dir: &Path) {
    let install_str = install_dir.display().to_string();
    if let Err(e) = user_env::remove_from_user_path(&install_str) {
        tracing::warn!("移除 composer PATH 失败: {}", e);
    }
}

#[cfg(target_os = "windows")]
fn unlink_composer(install_dir: &Path) -> UnlinkReport {
    let mut r = UnlinkReport::default();
    let install_str = install_dir.display().to_string();
    match user_env::remove_from_user_path(&install_str) {
        Ok(true) => r.cleared.push(format!("用户 PATH 条目: {}", install_str)),
        Ok(false) => {}
        Err(e) => r.errors.push(format!("清 PATH 失败: {}", e)),
    }
    r
}

#[cfg(target_os = "windows")]
fn deep_uninstall_composer(install_dir: &Path) -> DeepUninstallReport {
    let mut r = DeepUninstallReport::default();

    // 删 composer.* 文件，不删整个目录（目录可能跟其他工具共用）
    if let Ok(rd) = std::fs::read_dir(install_dir) {
        for entry in rd.flatten() {
            let fname = entry.file_name();
            let lower = fname.to_string_lossy().to_ascii_lowercase();
            let is_composer_file = lower == "composer"
                || lower.starts_with("composer.")
                || lower.starts_with("composer-");
            if !is_composer_file {
                continue;
            }
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            match std::fs::remove_file(&p) {
                Ok(()) => r.deleted_files.push(p.display().to_string()),
                Err(e) => r.errors.push(format!("删 {} 失败: {}", p.display(), e)),
            }
        }
    }

    // 清 PATH
    let install_str = install_dir.display().to_string();
    match user_env::remove_from_user_path(&install_str) {
        Ok(true) => r.cleared_env.push(format!("用户 PATH 条目: {}", install_str)),
        Ok(false) => {}
        Err(e) => r.errors.push(format!("清 PATH 失败: {}", e)),
    }

    // 列 COMPOSER_HOME 残余（不删，告诉用户）
    if let Ok(home) = std::env::var("USERPROFILE") {
        let composer_home = std::path::PathBuf::from(home)
            .join("AppData")
            .join("Roaming")
            .join("Composer");
        if composer_home.exists() {
            r.kept_paths.push(composer_home.display().to_string());
        }
    }

    r
}

// ──────────────────────────────────────────────────────────────────────────
// nvm-windows
// ──────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn nvm(install_dir: &Path) {
    let symlink_dir = install_dir.join("nodejs");
    let install_str = install_dir.display().to_string();
    let symlink_str = symlink_dir.display().to_string();

    // 1. settings.txt —— root + symlink path + 国内镜像源
    let settings = format!(
        "root: {root}\r\npath: {path}\r\nnode_mirror: https://npmmirror.com/mirrors/node/\r\nnpm_mirror: https://npmmirror.com/mirrors/npm/\r\n",
        root = install_str,
        path = symlink_str,
    );
    if let Err(e) = std::fs::write(install_dir.join("settings.txt"), settings.as_bytes()) {
        tracing::warn!("写 nvm settings.txt 失败: {}", e);
    }

    // 2. 用户环境变量 NVM_HOME + NVM_SYMLINK
    if let Err(e) = user_env::set_user_env_var("NVM_HOME", &install_str) {
        tracing::warn!("设 NVM_HOME 失败: {}", e);
    }
    if let Err(e) = user_env::set_user_env_var("NVM_SYMLINK", &symlink_str) {
        tracing::warn!("设 NVM_SYMLINK 失败: {}", e);
    }

    // 3. 用户 PATH 加两条
    if let Err(e) = user_env::append_to_user_path(&install_str) {
        tracing::warn!("加 NVM_HOME 到 PATH 失败: {}", e);
    }
    if let Err(e) = user_env::append_to_user_path(&symlink_str) {
        tracing::warn!("加 NVM_SYMLINK 到 PATH 失败: {}", e);
    }
}

#[cfg(target_os = "windows")]
fn nvm_uninstall(install_dir: &Path) {
    let symlink_dir = install_dir.join("nodejs");
    let install_str = install_dir.display().to_string();
    let symlink_str = symlink_dir.display().to_string();

    let _ = user_env::remove_from_user_path(&install_str);
    let _ = user_env::remove_from_user_path(&symlink_str);
    let _ = user_env::unset_user_env_var("NVM_HOME");
    let _ = user_env::unset_user_env_var("NVM_SYMLINK");
}

#[cfg(target_os = "windows")]
fn unlink_nvm(install_dir: &Path) -> UnlinkReport {
    let mut r = UnlinkReport::default();
    let symlink_dir = install_dir.join("nodejs");

    for (label, value) in [
        ("用户 PATH", install_dir.display().to_string()),
        ("用户 PATH (symlink)", symlink_dir.display().to_string()),
    ] {
        match user_env::remove_from_user_path(&value) {
            Ok(true) => r.cleared.push(format!("{}: {}", label, value)),
            Ok(false) => {}
            Err(e) => r.errors.push(format!("清 {} 失败: {}", label, e)),
        }
    }
    for var in ["NVM_HOME", "NVM_SYMLINK"] {
        match user_env::unset_user_env_var(var) {
            Ok(()) => r.cleared.push(format!("环境变量: {}", var)),
            Err(e) => r.errors.push(format!("清 {} 失败: {}", var, e)),
        }
    }
    r
}

/// nvm-noinstall.zip / nvm-setup.exe 装出来的核心本体文件清单。
/// 删这些文件就让 nvm 不可用，但不影响平级的 v*.*.* 版本目录。
#[cfg(target_os = "windows")]
const NVM_CORE_FILES: &[&str] = &[
    "nvm.exe",
    "elevate.cmd",
    "elevate.vbs",
    "setuserenv.vbs",
    "unsetuserenv.vbs",
    "settings.txt",
    "LICENSE",
    "README.md",
    "nvm.ico",
    "nodejs.ico",
    "alert.ico",
    "author.ico",
    "download.ico",
    "success.ico",
    "author-nvm.exe",
    "install.cmd",
    "run.cmd",
];

#[cfg(target_os = "windows")]
fn deep_uninstall_nvm(install_dir: &Path) -> DeepUninstallReport {
    let mut r = DeepUninstallReport::default();

    if let Ok(rd) = std::fs::read_dir(install_dir) {
        for entry in rd.flatten() {
            let fname = entry.file_name();
            let s = fname.to_string_lossy();
            let path = entry.path();
            let ftype = entry.file_type().ok();

            // node 版本目录（v20.18.0 / v18.19.1 / ...）—— 保留
            if ftype.is_some_and(|t| t.is_dir())
                && s.starts_with('v')
                && s.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
            {
                r.kept_paths.push(path.display().to_string());
                continue;
            }

            // current symlink (nodejs)
            if s == "nodejs" {
                // 是 symlink/junction，用 remove_dir 删（remove_file 对 dir symlink 也行但行为差异）
                let _ = std::fs::remove_dir(&path).or_else(|_| std::fs::remove_file(&path));
                r.deleted_files.push(path.display().to_string());
                continue;
            }

            // 核心本体文件 → 删
            if NVM_CORE_FILES.iter().any(|f| f.eq_ignore_ascii_case(&s)) {
                if path.is_file() {
                    match std::fs::remove_file(&path) {
                        Ok(()) => r.deleted_files.push(path.display().to_string()),
                        Err(e) => r.errors.push(format!("删 {} 失败: {}", path.display(), e)),
                    }
                }
            }
            // 其他不认识的：不动（避免误删用户自己放的东西）
        }
    }

    // 清环境
    let symlink_dir = install_dir.join("nodejs");
    let install_str = install_dir.display().to_string();
    let symlink_str = symlink_dir.display().to_string();
    for (label, value) in [("用户 PATH", &install_str), ("用户 PATH (symlink)", &symlink_str)]
    {
        match user_env::remove_from_user_path(value) {
            Ok(true) => r.cleared_env.push(format!("{}: {}", label, value)),
            Ok(false) => {}
            Err(e) => r.errors.push(format!("清 {} 失败: {}", label, e)),
        }
    }
    for var in ["NVM_HOME", "NVM_SYMLINK"] {
        match user_env::unset_user_env_var(var) {
            Ok(()) => r.cleared_env.push(format!("环境变量: {}", var)),
            Err(e) => r.errors.push(format!("清 {} 失败: {}", var, e)),
        }
    }

    r
}

#[cfg(target_os = "windows")]
fn naxone_php_shim() -> PathBuf {
    crate::platform::global_php::bin_dir().join("php.cmd")
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn naxone_php_shim() -> PathBuf {
    PathBuf::new()
}

// ──────────────────────────────────────────────────────────────────────────
// MySQL
// ──────────────────────────────────────────────────────────────────────────

/// 解压后跑：写 my.ini → mysqld --initialize-insecure → 临时启动 + ALTER USER
/// 设默认密码 "root"。失败按 best-effort 不阻塞主安装。
#[cfg(target_os = "windows")]
fn mysql(install_dir: &Path) {
    let bin = install_dir.join("bin");
    let mysqld = bin.join("mysqld.exe");
    if !mysqld.is_file() {
        tracing::warn!("mysql post-install: 找不到 {}", mysqld.display());
        return;
    }

    let basedir = install_dir.to_path_buf();
    let datadir = install_dir.join("data");
    let ini = install_dir.join("my.ini");

    if !ini.exists() {
        if let Err(e) = write_default_my_ini(&ini, &basedir, &datadir) {
            tracing::warn!("写 my.ini 失败: {}", e);
        }
    }

    // mysql 系统库子目录在 → 已经初始化过，不再重复
    let already_init = datadir.join("mysql").exists();
    if !already_init {
        if datadir.exists() {
            // 残留空目录或半成品，清掉让 mysqld 自己建
            let _ = std::fs::remove_dir_all(&datadir);
        }
        match run_blocking(
            &mysqld,
            &[
                &format!("--basedir={}", basedir.display()),
                &format!("--datadir={}", datadir.display()),
                "--initialize-insecure",
                "--console",
            ],
            std::time::Duration::from_secs(120),
        ) {
            Ok(_) => tracing::info!("MySQL data 已初始化: {}", datadir.display()),
            Err(e) => {
                tracing::warn!("MySQL --initialize-insecure 失败: {}", e);
                return;
            }
        }
    }

    // 设默认密码 root（只在还没设过时做一次）
    let pwd_file = install_dir.join(".naxone-root.txt");
    if !pwd_file.exists() {
        match set_root_password_via_init_file(&mysqld, &basedir, &datadir, &bin, "root") {
            Ok(()) => {
                let _ = std::fs::write(&pwd_file, b"root");
                tracing::info!("MySQL root 默认密码设为 root");
            }
            Err(e) => {
                tracing::warn!("MySQL 设默认密码失败（保留空密码，UI 可手动改）: {}", e);
                let _ = std::fs::write(&pwd_file, b"");
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn mysql_uninstall(install_dir: &Path) {
    // 数据目录与密码文件交给"彻底卸载"流程处理；这里无 PATH 注册可清。
    let _ = install_dir;
}

#[cfg(target_os = "windows")]
fn write_default_my_ini(
    ini: &Path,
    basedir: &Path,
    datadir: &Path,
) -> std::io::Result<()> {
    // mysql ini 在 Windows 下用反斜杠也能解析，但正斜杠更稳
    let basedir_s = basedir.display().to_string().replace('\\', "/");
    let datadir_s = datadir.display().to_string().replace('\\', "/");
    let content = format!(
        "[client]\r\n\
port=3306\r\n\
default-character-set=utf8mb4\r\n\
\r\n\
[mysqld]\r\n\
port=3306\r\n\
basedir={basedir}\r\n\
datadir={datadir}\r\n\
character-set-server=utf8mb4\r\n\
collation-server=utf8mb4_unicode_ci\r\n\
default-storage-engine=INNODB\r\n\
default_authentication_plugin=mysql_native_password\r\n\
max_connections=200\r\n\
max_allowed_packet=64M\r\n\
sql_mode=STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION\r\n",
        basedir = basedir_s,
        datadir = datadir_s,
    );
    std::fs::write(ini, content.as_bytes())
}

/// 临时启动 mysqld（非标准端口）+ --init-file 跑 ALTER USER，然后 mysqladmin shutdown。
#[cfg(target_os = "windows")]
fn set_root_password_via_init_file(
    mysqld: &Path,
    basedir: &Path,
    datadir: &Path,
    bin: &Path,
    new_pwd: &str,
) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let init_sql_path = std::env::temp_dir()
        .join(format!("naxone-mysql-init-{}.sql", std::process::id()));
    // 单引号转义：'  →  ''
    let escaped = new_pwd.replace('\'', "''");
    let init_sql = format!(
        "ALTER USER 'root'@'localhost' IDENTIFIED BY '{}';\nFLUSH PRIVILEGES;\n",
        escaped
    );
    std::fs::write(&init_sql_path, init_sql.as_bytes())
        .map_err(|e| format!("写 init.sql 失败: {}", e))?;

    let port = pick_free_port().unwrap_or(33099);

    // 启动 mysqld 后台进程（不带 my.ini，避免端口冲突）
    let mut cmd = std::process::Command::new(mysqld);
    cmd.args([
        format!("--basedir={}", basedir.display()),
        format!("--datadir={}", datadir.display()),
        format!("--port={}", port),
        format!("--init-file={}", init_sql_path.display()),
        "--skip-networking=0".into(),
        "--console".into(),
    ]);
    cmd.creation_flags(CREATE_NO_WINDOW);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动临时 mysqld 失败: {}", e))?;

    // 等端口可连接（最多 20 秒，初始化完的 mysqld 启动通常 2-5s）
    let connected = wait_port(port, std::time::Duration::from_secs(20));

    let shutdown_result = if connected {
        let mysqladmin = bin.join("mysqladmin.exe");
        let r = std::process::Command::new(&mysqladmin)
            .args([
                format!("-P{}", port).as_str(),
                "-h127.0.0.1",
                "-uroot",
                format!("-p{}", new_pwd).as_str(),
                "shutdown",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match r {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(format!(
                "mysqladmin shutdown 失败: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            )),
            Err(e) => Err(format!("mysqladmin 调用失败: {}", e)),
        }
    } else {
        Err("mysqld 未在 20s 内启动监听".into())
    };

    // 兜底：shutdown 失败时强杀子进程，避免卡住
    if shutdown_result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let _ = std::fs::remove_file(&init_sql_path);

    shutdown_result
}

/// 在 [33099, 33199) 找一个可绑定的端口
#[cfg(target_os = "windows")]
fn pick_free_port() -> Option<u16> {
    use std::net::TcpListener;
    for p in 33099u16..33199 {
        if TcpListener::bind(("127.0.0.1", p)).is_ok() {
            return Some(p);
        }
    }
    None
}

/// 轮询端口直到可连接或超时
#[cfg(target_os = "windows")]
fn wait_port(port: u16, timeout: std::time::Duration) -> bool {
    use std::net::TcpStream;
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            std::time::Duration::from_millis(500),
        )
        .is_ok()
        {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    false
}

/// 跑一个一次性命令，等它退出（可设超时）。隐藏控制台窗口。
#[cfg(target_os = "windows")]
fn run_blocking(
    exe: &Path,
    args: &[&str],
    timeout: std::time::Duration,
) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut child = std::process::Command::new(exe)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 {} 失败: {}", exe.display(), e))?;

    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                use std::io::Read;
                if let Some(mut o) = child.stdout.take() {
                    let _ = o.read_to_string(&mut stdout);
                }
                if let Some(mut e) = child.stderr.take() {
                    let _ = e.read_to_string(&mut stderr);
                }
                if status.success() {
                    return Ok(stdout);
                }
                return Err(format!("退出码 {:?}: {}", status.code(), stderr.trim()));
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    return Err(format!("超时（>{:?}）", timeout));
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(e) => return Err(format!("等待子进程失败: {}", e)),
        }
    }
}
