use std::path::{Path, PathBuf};

use crate::error::Result;

/// Read and write configuration files on the filesystem
pub trait ConfigIO: Send + Sync {
    /// Read a file as string
    fn read_text(&self, path: &Path) -> Result<String>;

    /// Write string content to a file
    fn write_text(&self, path: &Path, content: &str) -> Result<()>;

    /// Create a backup copy of a file (returns backup path)
    fn backup(&self, path: &Path) -> Result<PathBuf>;

    /// Check if a file exists
    fn exists(&self, path: &Path) -> bool;

    /// List files in a directory matching a pattern
    fn list_files(&self, dir: &Path, extension: &str) -> Result<Vec<PathBuf>>;

    /// Delete a file
    fn delete(&self, path: &Path) -> Result<()>;
}
