//! Package installer: stream-download → (optional SHA256 verify) →
//! unzip to staging → detect wrapper → move to PHPStudy-style dir.
//!
//! Install layout (phase-2 PHPStudy mimicry):
//!
//!   <packages_root>/
//!     _staging/
//!       {name}-{version}.zip       (temp, deleted after extract)
//!       {name}-{version}/          (temp, deleted after move)
//!     Nginx1.26.2/                 (final)
//!       nginx.exe
//!     MySQL8.0.40/                 (final)
//!       bin/mysqld.exe
//!     php/
//!       php842nts/                 (final, note: php has a shared parent dir)
//!         php-cgi.exe

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::UnboundedSender;

use super::manifest::{PackageEntry, PackageVersion};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "phase")]
pub enum InstallEvent {
    #[serde(rename = "started")]
    Started {
        name: String,
        version: String,
        total: Option<u64>,
    },
    #[serde(rename = "progress")]
    Progress {
        name: String,
        version: String,
        downloaded: u64,
        /// 总大小（字节）。服务器不给 Content-Length（如 chunked encoding）时为 None；
        /// 此时前端应展示已下载 MB 而不是百分比。
        total: Option<u64>,
        /// 已下载百分比。total 未知时恒为 0。
        pct: f32,
    },
    #[serde(rename = "extracting")]
    Extracting { name: String, version: String },
    #[serde(rename = "done")]
    Done {
        name: String,
        version: String,
        install_path: String,
    },
    #[serde(rename = "failed")]
    Failed {
        name: String,
        version: String,
        reason: String,
    },
}

pub struct Installer {
    client: reqwest::Client,
    packages_root: PathBuf,
}

impl Installer {
    pub fn new(packages_root: PathBuf) -> Self {
        let mut builder = reqwest::Client::builder()
            .user_agent(concat!("NaxOne/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(15))
            // 整体请求超时：避免国外镜像响应极慢时前端永远卡在"连接中"
            .timeout(std::time::Duration::from_secs(600))
            .pool_idle_timeout(std::time::Duration::from_secs(30));

        // 自动代理：用户开了 Clash/V2ray 的"系统代理"时读系统设置
        if let Some(proxy_url) = crate::package::proxy::detect_proxy() {
            match reqwest::Proxy::all(&proxy_url) {
                Ok(p) => {
                    tracing::info!(proxy = %proxy_url, "Installer 使用系统代理");
                    builder = builder.proxy(p);
                }
                Err(e) => {
                    tracing::warn!(proxy = %proxy_url, err = %e, "系统代理地址无效，回退直连");
                }
            }
        }

        let client = builder.build().expect("reqwest client builds");
        Self {
            client,
            packages_root,
        }
    }

    /// Install a package version. Emits progress events via `tx`.
    /// Returns the final install path on success.
    pub async fn install(
        &self,
        entry: &PackageEntry,
        version: &PackageVersion,
        tx: UnboundedSender<InstallEvent>,
    ) -> Result<PathBuf, String> {
        let name = entry.name.clone();
        let ver = version.version.clone();

        // Final destination: PHPStudy-style directory name.
        let final_name = phpstudy_style_dir_name(&name, &ver);
        let final_dir = self.packages_root.join(&final_name);

        // Idempotency: already installed?
        if final_dir.join(&version.exe_rel).exists() {
            let _ = tx.send(InstallEvent::Done {
                name: name.clone(),
                version: ver.clone(),
                install_path: final_dir.display().to_string(),
            });
            return Ok(final_dir);
        }

        // Guard: if the target directory exists but looks incomplete, make
        // sure we're not about to stomp on a running service's files.
        if final_dir.exists() {
            // For now we just delete and reinstall. If the service was running,
            // the OS file lock will fail the delete and the install will error.
            if let Err(e) = std::fs::remove_dir_all(&final_dir) {
                let msg = format!(
                    "目标目录已存在且无法删除（可能服务正在运行）: {} ({})",
                    final_dir.display(),
                    e
                );
                let _ = tx.send(InstallEvent::Failed {
                    name: name.clone(),
                    version: ver.clone(),
                    reason: msg.clone(),
                });
                return Err(msg);
            }
        }

        // Staging area for temp zip + unzip.
        let staging_root = self.packages_root.join("_staging");
        if let Err(e) = std::fs::create_dir_all(&staging_root) {
            return fail(&tx, &name, &ver, format!("创建 staging 目录失败: {}", e));
        }

        let temp_zip = staging_root.join(format!("{}-{}.zip", name, ver));
        let unpacked = staging_root.join(format!("{}-{}", name, ver));
        // Clean any leftover from a previous failed run
        let _ = std::fs::remove_dir_all(&unpacked);
        let _ = std::fs::remove_file(&temp_zip);

        // ---------- download (按候选 URL 顺序尝试) ----------
        let urls = version.candidate_urls();
        if urls.is_empty() {
            return fail(&tx, &name, &ver, "没有可用的下载 URL".to_string());
        }
        let mut download_err: Option<String> = None;
        for (idx, url) in urls.iter().enumerate() {
            // 清掉上次失败的残片，避免 File::create 直接拿到旧文件
            let _ = std::fs::remove_file(&temp_zip);
            tracing::info!(attempt = idx + 1, total = urls.len(), url = %url, "下载尝试");
            match self.download(&name, &ver, url, &temp_zip, &tx).await {
                Ok(()) => {
                    if let Err(e) = validate_zip_file(&temp_zip) {
                        tracing::warn!(url = %url, "下载内容不是有效 zip: {}", e);
                        download_err = Some(format!("{}（{}）", e, url));
                        continue;
                    }
                    tracing::info!(url = %url, "下载完成并通过 zip 校验");
                    download_err = None;
                    break;
                }
                Err(e) => {
                    tracing::warn!("镜像 {} 失败: {}", url, e);
                    download_err = Some(e);
                    // 还有下一个就继续
                }
            }
        }
        if let Some(e) = download_err {
            let _ = std::fs::remove_file(&temp_zip);
            return fail(
                &tx,
                &name,
                &ver,
                format!("所有下载源均失败，最后一次错误: {}", e),
            );
        }

        // ---------- SHA256 ----------
        if let Some(expected) = &version.sha256 {
            if let Err(e) = verify_sha256(&temp_zip, expected).await {
                let _ = std::fs::remove_file(&temp_zip);
                return fail(&tx, &name, &ver, e);
            }
        }

        finalize_install(entry, version, &name, &ver, &final_dir, &temp_zip, &unpacked, &tx)
    }
    async fn download(
        &self,
        name: &str,
        version: &str,
        url: &str,
        dest: &Path,
        tx: &UnboundedSender<InstallEvent>,
    ) -> Result<(), String> {
        let _ = tx.send(InstallEvent::Started {
            name: name.into(),
            version: version.into(),
            total: None,
        });

        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("网络请求失败（{}）: {}", url, e))?;

        let status = resp.status();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("<unknown>")
            .to_string();
        let total = resp.content_length();

        tracing::info!(
            url = %url,
            status = %status,
            content_type = %content_type,
            content_length = ?total,
            "下载响应信息"
        );

        if !status.is_success() {
            return Err(format!(
                "HTTP {}: {} (content-type: {})",
                status.as_u16(),
                url,
                content_type
            ));
        }

        if content_type.to_ascii_lowercase().starts_with("text/html") {
            return Err(format!(
                "下载响应为 HTML（疑似拦截页/错误页），非安装包: {}",
                url
            ));
        }

        if total.is_some() {
            let _ = tx.send(InstallEvent::Started {
                name: name.into(),
                version: version.into(),
                total,
            });
        }

        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| format!("创建临时文件失败: {}", e))?;

        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_emit_bytes: u64 = 0;
        let mut last_emit_at = std::time::Instant::now();
        const EMIT_BYTES_THRESHOLD: u64 = 256 * 1024;
        const EMIT_TIME_THRESHOLD_MS: u128 = 300;

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| format!("下载中断: {}", e))?;
            file.write_all(&bytes)
                .await
                .map_err(|e| format!("写入失败: {}", e))?;
            downloaded += bytes.len() as u64;

            let enough_bytes = downloaded - last_emit_bytes >= EMIT_BYTES_THRESHOLD;
            let enough_time = last_emit_at.elapsed().as_millis() >= EMIT_TIME_THRESHOLD_MS;
            if enough_bytes || enough_time {
                last_emit_bytes = downloaded;
                last_emit_at = std::time::Instant::now();
                let pct = match total {
                    Some(t) if t > 0 => (downloaded as f32 / t as f32 * 100.0).min(100.0),
                    _ => 0.0,
                };
                let _ = tx.send(InstallEvent::Progress {
                    name: name.into(),
                    version: version.into(),
                    downloaded,
                    total,
                    pct,
                });
            }
        }

        file.flush().await.map_err(|e| format!("刷盘失败: {}", e))?;

        tracing::info!(
            url = %url,
            bytes = downloaded,
            content_type = %content_type,
            "下载完成"
        );

        let final_pct = match total {
            Some(t) if t > 0 => 100.0,
            _ => 0.0,
        };
        let _ = tx.send(InstallEvent::Progress {
            name: name.into(),
            version: version.into(),
            downloaded,
            total,
            pct: final_pct,
        });

        Ok(())
    }
}

/// PHPStudy-style destination directory for a given package.
///
///   nginx  1.26.2  → "Nginx1.26.2"
///   mysql  8.0.40  → "MySQL8.0.40"
///   apache 2.4.62  → "Apache2.4.62"
///   redis  5.0.14.1 → "Redis5.0.14.1"
///   php    8.4.2   → "php/php842nts"
pub fn phpstudy_style_dir_name(name: &str, version: &str) -> String {
    match name {
        "nginx" => format!("Nginx{}", version),
        "apache" => format!("Apache{}", version),
        "mysql" => format!("MySQL{}", version),
        "redis" => format!("Redis{}", version),
        "php" => format!("php/php{}nts", version.replace('.', "")),
        other => format!("{}{}", other, version),
    }
}

/// Walk the unpacked tree to find the directory that actually contains `exe_rel`.
/// Handles nested wrappers (e.g. Apache Lounge zips have httpd-xxx/Apache24/bin/httpd.exe).
/// Falls back to the unpacked root if nothing matches.
fn find_exe_root(unpacked: &Path, exe_rel: &str) -> PathBuf {
    if unpacked.join(exe_rel).exists() {
        return unpacked.to_path_buf();
    }
    // Search up to 3 levels deep for the exe
    fn search(dir: &Path, exe_rel: &str, depth: u32) -> Option<PathBuf> {
        if depth == 0 { return None; }
        let rd = std::fs::read_dir(dir).ok()?;
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join(exe_rel).exists() {
                return Some(path);
            }
        }
        // Recurse into single-subdir wrappers
        let rd = std::fs::read_dir(dir).ok()?;
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = search(&path, exe_rel, depth - 1) {
                    return Some(found);
                }
            }
        }
        None
    }
    search(unpacked, exe_rel, 3).unwrap_or_else(|| unpacked.to_path_buf())
}

/// If `dir` contains exactly one subdirectory and no regular files, return
/// the path to that subdirectory. Otherwise None.
fn unwrap_single_subdir(dir: &Path) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut only_subdir: Option<PathBuf> = None;
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if only_subdir.is_some() {
                // More than one subdir → not a wrapper
                return None;
            }
            only_subdir = Some(path);
        } else {
            // Any file means this is already the install root
            return None;
        }
    }
    only_subdir
}

async fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let data = tokio::fs::read(path)
        .await
        .map_err(|e| format!("读取校验文件失败: {}", e))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let got = format!("{:x}", hasher.finalize());
    if got.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "SHA256 校验失败: 期望 {}, 实际 {}",
            expected, got
        ))
    }
}

/// Extract a zip file. Uses zip crate's enclosed_name() which honors the
/// utf-8 flag and strips zip-slip traversal.
fn validate_zip_file(path: &Path) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("打开下载文件失败: {}", e))?;
    let _ = zip::ZipArchive::new(file).map_err(|e| format!("下载文件不是有效 zip: {}", e))?;
    Ok(())
}

fn unzip(src: &Path, dst: &Path) -> Result<(), String> {
    let file = std::fs::File::open(src).map_err(|e| format!("打开 zip 失败: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("解析 zip 失败: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 zip 条目 {} 失败: {}", i, e))?;
        let rel_path = match entry.enclosed_name() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        let out_path = dst.join(rel_path);

        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&out_path);
            continue;
        }
        if let Some(p) = out_path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let mut out = std::fs::File::create(&out_path)
            .map_err(|e| format!("创建 {} 失败: {}", out_path.display(), e))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| format!("写入 {} 失败: {}", out_path.display(), e))?;
    }

    Ok(())
}

/// Recursive copy. Used as a fallback when `rename` fails (e.g. cross-volume).
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn fail(
    tx: &UnboundedSender<InstallEvent>,
    name: &str,
    version: &str,
    reason: String,
) -> Result<PathBuf, String> {
    let _ = tx.send(InstallEvent::Failed {
        name: name.into(),
        version: version.into(),
        reason: reason.clone(),
    });
    Err(reason)
}

fn finalize_install(
    entry: &PackageEntry,
    version: &PackageVersion,
    name: &str,
    ver: &str,
    final_dir: &Path,
    temp_zip: &Path,
    unpacked: &Path,
    tx: &UnboundedSender<InstallEvent>,
) -> Result<PathBuf, String> {
    let _ = tx.send(InstallEvent::Extracting {
        name: name.to_string(),
        version: ver.to_string(),
    });
    if let Err(e) = std::fs::create_dir_all(unpacked) {
        let _ = std::fs::remove_file(temp_zip);
        return fail(tx, name, ver, format!("创建解压目录失败: {}", e));
    }
    if let Err(e) = unzip(temp_zip, unpacked) {
        let _ = std::fs::remove_dir_all(unpacked);
        let _ = std::fs::remove_file(temp_zip);
        return fail(tx, name, ver, e);
    }
    let _ = std::fs::remove_file(temp_zip);

    let source_dir = find_exe_root(unpacked, &version.exe_rel);

    if let Some(parent) = final_dir.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::rename(&source_dir, final_dir) {
        tracing::warn!(
            "rename failed ({}), falling back to copy: {} → {}",
            e,
            source_dir.display(),
            final_dir.display()
        );
        if let Err(e2) = copy_dir_all(&source_dir, final_dir) {
            let _ = std::fs::remove_dir_all(final_dir);
            let _ = std::fs::remove_dir_all(unpacked);
            return fail(tx, name, ver, format!("移动到最终目录失败: {}", e2));
        }
        let _ = std::fs::remove_dir_all(&source_dir);
    }

    let _ = std::fs::remove_dir_all(unpacked);

    let exe_final = final_dir.join(&version.exe_rel);
    if !exe_final.exists() {
        let _ = std::fs::remove_dir_all(final_dir);
        return fail(
            tx,
            name,
            ver,
            format!(
                "解压后未找到预期文件 {} (检查 exe_rel 或 zip 结构)",
                exe_final.display()
            ),
        );
    }

    if entry.name == "php" {
        let ini = final_dir.join("php.ini");
        if !ini.exists() {
            let production = final_dir.join("php.ini-production");
            let development = final_dir.join("php.ini-development");
            let template = if production.exists() {
                Some(production)
            } else if development.exists() {
                Some(development)
            } else {
                None
            };
            if let Some(src) = template {
                if let Err(e) = std::fs::copy(&src, &ini) {
                    tracing::warn!("生成 php.ini 失败 (从 {}): {}", src.display(), e);
                } else {
                    tracing::info!(
                        "已生成 php.ini（来自 {}）",
                        src.file_name().unwrap_or_default().to_string_lossy()
                    );
                }
            } else {
                tracing::warn!("找不到 php.ini-production/development 模板，跳过 ini 初始化");
            }
        }
    }

    let _ = tx.send(InstallEvent::Done {
        name: name.to_string(),
        version: ver.to_string(),
        install_path: final_dir.display().to_string(),
    });
    Ok(final_dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phpstudy_dir_names() {
        assert_eq!(phpstudy_style_dir_name("nginx", "1.26.2"), "Nginx1.26.2");
        assert_eq!(phpstudy_style_dir_name("apache", "2.4.62"), "Apache2.4.62");
        assert_eq!(phpstudy_style_dir_name("mysql", "8.0.40"), "MySQL8.0.40");
        assert_eq!(phpstudy_style_dir_name("redis", "5.0.14.1"), "Redis5.0.14.1");
        assert_eq!(phpstudy_style_dir_name("php", "8.4.2"), "php/php842nts");
        assert_eq!(phpstudy_style_dir_name("php", "8.3.30"), "php/php8330nts");
        assert_eq!(phpstudy_style_dir_name("php", "7.4.33"), "php/php7433nts");
    }

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir()
            .join("naxone-installer-test")
            .join(format!("{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn unwrap_detects_single_subdir() {
        let root = tmp("wrapper");
        let inner = root.join("nginx-1.26.2");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(inner.join("nginx.exe"), b"").unwrap();

        assert_eq!(unwrap_single_subdir(&root), Some(inner));
    }

    #[test]
    fn unwrap_returns_none_when_files_present() {
        let root = tmp("no-wrapper");
        std::fs::write(root.join("redis-server.exe"), b"").unwrap();
        std::fs::create_dir_all(root.join("dir")).unwrap();
        assert_eq!(unwrap_single_subdir(&root), None);
    }

    #[test]
    fn unwrap_returns_none_when_multiple_subdirs() {
        let root = tmp("multi");
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        assert_eq!(unwrap_single_subdir(&root), None);
    }
}
