//! Auto-discovers services by querying running Windows Services.
//!
//! Uses `sc query` + `sc qc` to find already-running services whose names
//! match known patterns (mysql, apache, redis, nginx). No directory scanning.

use std::path::PathBuf;

use ruststudy_core::domain::service::{ServiceInstance, ServiceKind, ServiceOrigin};

pub struct SystemScanner;

impl SystemScanner {
    pub fn scan() -> Vec<ServiceInstance> {
        let mut out: Vec<ServiceInstance> = Vec::new();

        for (svc_name, kind) in discover_running_services() {
            if let Some(inst) = build_instance(&svc_name, kind) {
                push_dedup(&mut out, inst);
            }
        }

        out
    }
}

// ── Service name → kind ────────────────────────────────────────────

const PATTERNS: &[(&str, ServiceKind)] = &[
    ("mysql", ServiceKind::Mysql),
    ("mariadb", ServiceKind::Mysql),
    ("apache", ServiceKind::Apache),
    ("httpd", ServiceKind::Apache),
    ("redis", ServiceKind::Redis),
    ("nginx", ServiceKind::Nginx),
];

fn classify(name: &str) -> Option<ServiceKind> {
    let lower = name.to_lowercase();
    for (pat, kind) in PATTERNS {
        if lower.contains(pat) {
            return Some(*kind);
        }
    }
    None
}

// ── sc query / sc qc ───────────────────────────────────────────────

fn discover_running_services() -> Vec<(String, ServiceKind)> {
    let output = run("sc", &["query", "type=", "service", "state=", "all"]);

    let mut results = Vec::new();
    let mut name: Option<String> = None;
    let mut running = false;

    for line in output.lines() {
        let line = line.trim();
        if let Some(n) = line.strip_prefix("SERVICE_NAME:") {
            // flush previous
            if running {
                if let Some(n) = name.take() {
                    if let Some(kind) = classify(&n) {
                        results.push((n, kind));
                    }
                }
            }
            name = Some(n.trim().to_string());
            running = false;
        } else if line.contains("STATE") && line.contains("RUNNING") {
            running = true;
        }
    }

    // flush last
    if running {
        if let Some(n) = name {
            if let Some(kind) = classify(&n) {
                results.push((n, kind));
            }
        }
    }

    results
}

fn build_instance(svc_name: &str, kind: ServiceKind) -> Option<ServiceInstance> {
    let qc_output = run("sc", &["qc", svc_name]);

    // Find: BINARY_PATH_NAME   : "C:\...\exe" args
    let bin_line = qc_output
        .lines()
        .find(|l| l.contains("BINARY_PATH_NAME"))?;

    let exe_path = extract_exe_path(bin_line)?;

    // Determine install root from exe location
    let install_root = match kind {
        ServiceKind::Nginx | ServiceKind::Redis | ServiceKind::Php => {
            exe_path.parent()?.to_path_buf()
        }
        ServiceKind::Apache | ServiceKind::Mysql => {
            // exe lives in bin/
            exe_path.parent()?.parent()?.to_path_buf()
        }
    };

    // Skip if inside PHPStudy / RustStudy tree
    let s = install_root.to_string_lossy().to_lowercase();
    if (s.contains("phpstudy") && s.contains("extensions"))
        || (s.contains("ruststudy") && s.contains("extensions"))
        || (s.contains("ruststudy") && s.contains("packages"))
    {
        return None;
    }

    super::standalone::probe_install(&install_root, kind, ServiceOrigin::System)
}

// ── Helpers ────────────────────────────────────────────────────────

fn extract_exe_path(line: &str) -> Option<PathBuf> {
    // "        BINARY_PATH_NAME   : \"C:\\path\\exe\" args"
    let after_key = line.split("BINARY_PATH_NAME").nth(1)?;
    let after_colon = after_key.trim_start_matches(|c: char| c == ' ' || c == ':');

    let path = if after_colon.starts_with('"') {
        let end = after_colon[1..].find('"')?;
        &after_colon[1..end + 1]
    } else {
        after_colon.split_whitespace().next()?
    };

    Some(PathBuf::from(path))
}

fn run(exe: &str, args: &[&str]) -> String {
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let mut cmd = std::process::Command::new(exe);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    match cmd.output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => String::new(),
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
