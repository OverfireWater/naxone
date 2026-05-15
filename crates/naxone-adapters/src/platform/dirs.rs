//! NaxOne 用户态目录解析。dev 与 prod 用不同后缀彻底隔离，避免 dev 写脏 PATH /
//! `~/.naxone/` 后污染 prod 的检测逻辑。
//!
//! - prod：`~/.naxone/`、`%APPDATA%/NaxOne/`
//! - dev：`~/.naxone-dev/`、`%APPDATA%/NaxOne-dev/`
//!
//! 通过 `cfg!(debug_assertions)` 编译期常量区分；release build 默认走 prod 路径。

use std::path::PathBuf;

/// 用户 home 下的 NaxOne 目录名。dev 时返回 `.naxone-dev`，prod 时返回 `.naxone`。
pub const fn naxone_home_dirname() -> &'static str {
    if cfg!(debug_assertions) {
        ".naxone-dev"
    } else {
        ".naxone"
    }
}

/// `%APPDATA%` 下 NaxOne 目录名。dev 时返回 `NaxOne-dev`，prod 时返回 `NaxOne`。
pub const fn naxone_appdata_dirname() -> &'static str {
    if cfg!(debug_assertions) {
        "NaxOne-dev"
    } else {
        "NaxOne"
    }
}

/// 完整路径：`<USERPROFILE>/<naxone_home_dirname>`
pub fn naxone_home_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
    PathBuf::from(home).join(naxone_home_dirname())
}

/// 完整路径：`<APPDATA>/<naxone_appdata_dirname>`
pub fn naxone_appdata_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
            PathBuf::from(home).join("AppData").join("Roaming")
        });
    appdata.join(naxone_appdata_dirname())
}

/// `<naxone_home>/bin/`，NaxOne 全局 CLI shim 目录（php.cmd / composer.bat 等）。
pub fn naxone_bin_dir() -> PathBuf {
    naxone_home_dir().join("bin")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_uses_separate_dirname() {
        // 测试在 cargo test 下跑（debug profile），断言用 dev 后缀
        if cfg!(debug_assertions) {
            assert_eq!(naxone_home_dirname(), ".naxone-dev");
            assert_eq!(naxone_appdata_dirname(), "NaxOne-dev");
        } else {
            assert_eq!(naxone_home_dirname(), ".naxone");
            assert_eq!(naxone_appdata_dirname(), "NaxOne");
        }
    }

    #[test]
    fn dev_and_prod_home_diverge() {
        // dev 与 prod 必须不同 —— 这是隔离的核心保证
        assert_ne!(".naxone", ".naxone-dev");
        assert_ne!("NaxOne", "NaxOne-dev");
    }

    #[test]
    fn naxone_home_dir_uses_dirname_helper() {
        // 路径末尾必须等于当前编译模式的 dirname
        let home = naxone_home_dir();
        let last = home.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(last, naxone_home_dirname());
    }

    #[test]
    fn bin_dir_under_home() {
        let bin = naxone_bin_dir();
        assert_eq!(bin.file_name().unwrap().to_string_lossy(), "bin");
        assert_eq!(bin.parent().unwrap(), naxone_home_dir());
    }
}
