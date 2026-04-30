//! 本地开发用的自签证书生成。
//!
//! 用 `rcgen` crate 生成含 SAN 的自签 X.509 证书 + 私钥，以 PEM 格式写入磁盘。
//! 有效期 825 天（CAB Forum 规定浏览器对自签证书的最大接受期）。
//!
//! SAN 列表包含：server_name + 用户给的 aliases + localhost + 127.0.0.1。
//! 浏览器首次访问仍会警告（除非用户手动导入到系统信任存储，比如 mkcert），
//! 但可以手动继续，满足 PWA / WebAuthn / 摄像头等 "require HTTPS" 开发场景。

use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

use rcgen::{
    CertificateParams, DistinguishedName, DnType, KeyPair, SanType,
};

/// 生成自签证书并写盘。返回 (cert_path, key_path)。
pub fn generate_self_signed(
    server_name: &str,
    aliases: &[String],
    out_dir: &Path,
) -> Result<(PathBuf, PathBuf), String> {
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("创建证书目录失败 {}: {}", out_dir.display(), e))?;

    // 构建 SAN：server_name + aliases + localhost + 127.0.0.1（去重）
    let mut sans: Vec<SanType> = Vec::new();
    let mut seen_dns: std::collections::HashSet<String> = std::collections::HashSet::new();
    let push_dns = |name: &str, sans: &mut Vec<SanType>, seen: &mut std::collections::HashSet<String>| {
        let n = name.trim().to_ascii_lowercase();
        if n.is_empty() { return; }
        if seen.insert(n.clone()) {
            if let Ok(parsed) = rcgen::Ia5String::try_from(n) {
                sans.push(SanType::DnsName(parsed));
            }
        }
    };
    push_dns(server_name, &mut sans, &mut seen_dns);
    for a in aliases {
        push_dns(a, &mut sans, &mut seen_dns);
    }
    push_dns("localhost", &mut sans, &mut seen_dns);
    sans.push(SanType::IpAddress(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));

    // Distinguished Name: CN = server_name
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, server_name);
    dn.push(DnType::OrganizationName, "NaxOne Local Dev");

    let mut params = CertificateParams::default();
    params.distinguished_name = dn;
    params.subject_alt_names = sans;
    // 有效期 825 天
    let now = time::OffsetDateTime::now_utc();
    params.not_before = now - time::Duration::days(1);
    params.not_after = now + time::Duration::days(825);

    let key = KeyPair::generate().map_err(|e| format!("生成密钥对失败: {}", e))?;
    let cert = params
        .self_signed(&key)
        .map_err(|e| format!("签发证书失败: {}", e))?;

    let cert_pem = cert.pem();
    let key_pem = key.serialize_pem();

    // 规范化文件名（去掉不适合文件系统的字符）
    let safe_name = sanitize_filename(server_name);
    let cert_path = out_dir.join(format!("{}.crt", safe_name));
    let key_path = out_dir.join(format!("{}.key", safe_name));

    std::fs::write(&cert_path, cert_pem)
        .map_err(|e| format!("写证书失败 {}: {}", cert_path.display(), e))?;
    std::fs::write(&key_path, key_pem)
        .map_err(|e| format!("写私钥失败 {}: {}", key_path.display(), e))?;

    Ok((cert_path, key_path))
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' | '_' => c,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_filename_keeps_safe_chars() {
        assert_eq!(sanitize_filename("foo.test"), "foo.test");
        assert_eq!(sanitize_filename("my-site_v1"), "my-site_v1");
        assert_eq!(sanitize_filename("a/b\\c:d"), "a_b_c_d");
    }

    #[test]
    fn generate_self_signed_produces_valid_files() {
        let tmp = std::env::temp_dir().join(format!("naxone_ssl_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (cert, key) = generate_self_signed("example.test", &["www.example.test".into()], &tmp)
            .expect("should succeed");
        assert!(cert.exists());
        assert!(key.exists());
        let cert_text = std::fs::read_to_string(&cert).unwrap();
        assert!(cert_text.starts_with("-----BEGIN CERTIFICATE-----"));
        let key_text = std::fs::read_to_string(&key).unwrap();
        assert!(key_text.contains("PRIVATE KEY"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
