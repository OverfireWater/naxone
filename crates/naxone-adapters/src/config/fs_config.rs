use std::path::{Path, PathBuf};

use naxone_core::error::{Result, NaxOneError};
use naxone_core::ports::config_io::ConfigIO;

/// Filesystem-based configuration reader/writer
pub struct FsConfigIO;

impl ConfigIO for FsConfigIO {
    fn read_text(&self, path: &Path) -> Result<String> {
        std::fs::read_to_string(path).map_err(|e| {
            NaxOneError::Config(format!("Failed to read {}: {}", path.display(), e))
        })
    }

    fn write_text(&self, path: &Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // 原子写：先写到同目录的 .tmp，写完再 rename 覆盖原文件。
        // 优势：写入中途崩溃不会留下"半个"配置文件；rename 在同一卷上是原子的。
        let tmp = path.with_extension("naxone.tmp");
        std::fs::write(&tmp, content).map_err(|e| {
            NaxOneError::Config(format!("Failed to write tmp {}: {}", tmp.display(), e))
        })?;
        // Windows 上 rename 到已存在文件需要 std::fs::rename 即可（原子覆盖）
        std::fs::rename(&tmp, path).map_err(|e| {
            // 失败时清理 tmp，避免遗留
            let _ = std::fs::remove_file(&tmp);
            NaxOneError::Config(format!("Failed to rename {} -> {}: {}", tmp.display(), path.display(), e))
        })
    }

    fn backup(&self, path: &Path) -> Result<PathBuf> {
        let backup_path = path.with_extension("bak");
        std::fs::copy(path, &backup_path)?;
        Ok(backup_path)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn list_files(&self, dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if dir.exists() {
            for entry in std::fs::read_dir(dir)?.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if ext == extension {
                            files.push(path);
                        }
                    }
                }
            }
        }
        Ok(files)
    }

    fn delete(&self, path: &Path) -> Result<()> {
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| {
                NaxOneError::Config(format!("Failed to delete {}: {}", path.display(), e))
            })
        } else {
            Ok(())
        }
    }
}
