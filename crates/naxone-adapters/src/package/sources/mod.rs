//! External version sources for the in-app software store.
//!
//! Each `VersionSource` fetches the current list of available versions from
//! an upstream (official JSON API / GitHub Releases / HTML scrape). Failures
//! bubble up as `Err(String)` so the caller can fall back to the embedded
//! manifest without propagating a hard error.

pub mod github_mirror;
pub mod php_official;
