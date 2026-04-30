//! Simple file-backed cache with TTL + stale-fallback support.
//!
//! Used by the software store to avoid hammering upstream version APIs on
//! every store-page open. Typical lifecycle:
//!
//!   1. `read_fresh()` → if the file exists and is within TTL, return it.
//!   2. Otherwise call the upstream API.
//!   3. On upstream success, `write()` the new value so subsequent opens are
//!      instant until the TTL elapses again.
//!   4. On upstream failure, `read_stale()` returns whatever's on disk even
//!      if it's past the TTL — better than nothing when the network's out.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{de::DeserializeOwned, Serialize};

pub struct DiskCache {
    path: PathBuf,
    ttl: Duration,
}

impl DiskCache {
    pub fn new(path: PathBuf, ttl: Duration) -> Self {
        Self { path, ttl }
    }

    /// Returns the cached value if the file exists AND was modified within
    /// `ttl`. Any I/O or parse error is swallowed and returns `None`.
    pub fn read_fresh<T: DeserializeOwned>(&self) -> Option<T> {
        let meta = std::fs::metadata(&self.path).ok()?;
        let mtime = meta.modified().ok()?;
        let age = SystemTime::now().duration_since(mtime).ok()?;
        if age > self.ttl {
            return None;
        }
        self.read_stale()
    }

    /// Returns the cached value ignoring TTL. Useful as a fallback when the
    /// upstream is unreachable.
    pub fn read_stale<T: DeserializeOwned>(&self) -> Option<T> {
        let data = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Persist a value to disk. Creates parent dirs as needed. Errors are
    /// propagated so the caller can log them, but the install flow itself
    /// never fails due to a cache write failure.
    pub fn write<T: Serialize>(&self, value: &T) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建缓存目录失败: {}", e))?;
        }
        let data = serde_json::to_string(value).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, data).map_err(|e| format!("写入缓存失败: {}", e))
    }

    /// Delete the cached file (if any). Used by "force refresh" flows.
    pub fn invalidate(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir()
            .join("naxone-cache-test")
            .join(format!("{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p.join("cache.json")
    }

    #[test]
    fn write_and_read_fresh() {
        let c = DiskCache::new(tmp("roundtrip"), Duration::from_secs(60));
        c.write(&vec![1, 2, 3]).unwrap();
        let got: Option<Vec<i32>> = c.read_fresh();
        assert_eq!(got, Some(vec![1, 2, 3]));
    }

    #[test]
    fn fresh_returns_none_if_missing() {
        let c = DiskCache::new(tmp("missing"), Duration::from_secs(60));
        let got: Option<Vec<i32>> = c.read_fresh();
        assert_eq!(got, None);
    }

    #[test]
    fn stale_still_readable_after_ttl() {
        // Write with zero-TTL cache, then read stale
        let path = tmp("stale");
        let c = DiskCache::new(path.clone(), Duration::from_nanos(1));
        c.write(&"hi".to_string()).unwrap();
        // Sleep a tick to guarantee TTL expiry
        std::thread::sleep(Duration::from_millis(5));
        let fresh: Option<String> = c.read_fresh();
        assert!(fresh.is_none(), "should be expired");
        let stale: Option<String> = c.read_stale();
        assert_eq!(stale.as_deref(), Some("hi"));
    }

    #[test]
    fn invalidate_removes_file() {
        let c = DiskCache::new(tmp("invalidate"), Duration::from_secs(60));
        c.write(&42u32).unwrap();
        c.invalidate();
        let got: Option<u32> = c.read_stale();
        assert_eq!(got, None);
    }
}
