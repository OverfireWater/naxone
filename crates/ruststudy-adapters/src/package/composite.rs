//! Unifies service discovery across all sources:
//!   1. PHPStudy's own `Extensions/` directory (if configured)
//!   2. RustStudy's own `Extensions/` directory (same format, different root)
//!   3. Extra install paths manually registered in config
//!   4. Legacy `%APPDATA%/RustStudy/Packages/{name}/{version}/` from pre-v2 store
//!
//! (1) and (2) both follow PHPStudy naming conventions (Nginx1.26.2, php/php842nts, ...)
//! so both can be scanned by `PhpStudyScanner`. We differentiate via `origin`
//! after the fact.
//!
//! De-dup strategy: `(kind, install_path)` must be unique. PHPStudy-origin
//! entries beat standalone-origin entries on collision (more metadata, more
//! trusted).

use std::path::Path;

use ruststudy_core::config::ExtraInstallPath;
use ruststudy_core::domain::service::{ServiceInstance, ServiceOrigin};

use super::scanner::PhpStudyScanner;
use super::standalone::StandaloneScanner;

pub struct CompositeScanner;

impl CompositeScanner {
    /// Scan all known sources.
    ///
    /// * `phpstudy_extensions` — `{phpstudy_path}/Extensions/` when the user
    ///   has PHPStudy installed. Entries scanned here get
    ///   `ServiceOrigin::PhpStudy`.
    /// * `store_extensions` — our own `Extensions/` (same layout convention).
    ///   Entries scanned here get `ServiceOrigin::Store`.
    /// * `extras` — user-specified extra install paths. Entries get
    ///   `ServiceOrigin::Manual`.
    /// * `legacy_packages_root` — the old v1 `%APPDATA%/RustStudy/Packages/`
    ///   layout, scanned for back-compat only.
    pub fn scan(
        phpstudy_extensions: Option<&Path>,
        store_extensions: Option<&Path>,
        extras: &[ExtraInstallPath],
        legacy_packages_root: Option<&Path>,
    ) -> Vec<ServiceInstance> {
        let mut out: Vec<ServiceInstance> = Vec::new();

        // --- 1) PHPStudy native Extensions ---
        if let Some(ext) = phpstudy_extensions {
            if let Ok(mut list) = PhpStudyScanner::scan(ext) {
                for inst in list.drain(..) {
                    push_dedup(&mut out, inst);
                }
            }
        }

        // --- 2) RustStudy store Extensions (same PHPStudy layout) ---
        if let Some(ext) = store_extensions {
            if let Ok(list) = PhpStudyScanner::scan(ext) {
                for mut inst in list {
                    // Rebrand origin since this root is ours, not PhpStudy's.
                    inst.origin = ServiceOrigin::Store;
                    push_dedup(&mut out, inst);
                }
            }
        }

        // --- 3) Manual extra_install_paths + legacy %APPDATA%/Packages ---
        let standalone = StandaloneScanner::scan(extras, legacy_packages_root);
        for inst in standalone {
            push_dedup(&mut out, inst);
        }

        // Re-number PHP ports for anything that wasn't laid out with a fixed
        // port by PhpStudyScanner (standalone / store imports start at 9000
        // and skip used ports).
        reassign_php_ports(&mut out);

        tracing::info!(count = out.len(), "CompositeScanner discovered services");
        out
    }
}

fn push_dedup(out: &mut Vec<ServiceInstance>, inst: ServiceInstance) {
    let already = out
        .iter()
        .any(|e| e.kind == inst.kind && e.install_path == inst.install_path);
    if !already {
        out.push(inst);
    }
}

fn reassign_php_ports(out: &mut [ServiceInstance]) {
    use ruststudy_core::domain::service::ServiceKind;

    // First pass: collect ports currently in use so we can avoid collisions.
    let mut used: std::collections::HashSet<u16> = out
        .iter()
        .filter(|s| s.kind == ServiceKind::Php)
        .map(|s| s.port)
        .collect();

    // We only rewrite ports for services whose current port conflicts with
    // something already in the set. PhpStudy entries got sensible ports
    // from PhpStudyScanner's 9000++ counter; store-origin entries also got
    // 9000++ from the same scanner — so after the merge we may have two
    // PHPs both on 9000. Bump the later ones.
    let mut seen: std::collections::HashSet<u16> = std::collections::HashSet::new();
    for inst in out.iter_mut() {
        if inst.kind != ServiceKind::Php {
            continue;
        }
        if seen.contains(&inst.port) {
            // Find next free
            let mut p = 9000u16;
            while used.contains(&p) {
                p += 1;
            }
            inst.port = p;
            used.insert(p);
        }
        seen.insert(inst.port);
    }
}
