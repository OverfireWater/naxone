use thiserror::Error;

#[derive(Debug, Error)]
pub enum NaxOneError {
    #[error("Service error: {0}")]
    Service(String),

    #[error("Process error: {0}")]
    Process(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Package error: {0}")]
    Package(String),

    #[error("Template error: {0}")]
    Template(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    /// 权限不足（例如写 hosts 需要管理员）
    #[error("需要管理员权限: {0}")]
    PermissionDenied(String),

    /// 文件被其他进程占用/锁定
    #[error("文件被占用: {0}")]
    FileLocked(String),

    /// 配置语法错误（nginx -t / httpd -t 失败等）
    #[error("配置语法错误: {0}")]
    ConfigSyntax(String),

    /// 端口已被占用
    #[error("端口 {port} 已被占用{}", .by.as_ref().map(|s| format!("（{}）", s)).unwrap_or_default())]
    PortInUse { port: u16, by: Option<String> },
}

impl NaxOneError {
    /// 根据 io::Error 的 ErrorKind 自动分类：权限/占用/其它
    pub fn from_io_with_context(err: std::io::Error, context: &str) -> Self {
        use std::io::ErrorKind;
        match err.kind() {
            ErrorKind::PermissionDenied => {
                NaxOneError::PermissionDenied(format!("{}: {}", context, err))
            }
            // Windows 上文件被占用通常是 ErrorKind::Other + os error 32
            _ => {
                let os = err.raw_os_error();
                if os == Some(32) || os == Some(33) {
                    NaxOneError::FileLocked(format!("{}: {}", context, err))
                } else {
                    NaxOneError::Io(err)
                }
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, NaxOneError>;
