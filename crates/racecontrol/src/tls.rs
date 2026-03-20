//! TLS certificate generation and RustlsConfig loader.
//!
//! Generates self-signed certificates via rcgen for the server's LAN IP,
//! and loads them into an axum-server RustlsConfig for HTTPS termination.

use axum_server::tls_rustls::RustlsConfig;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use std::path::Path;

const DEFAULT_CERT_DIR: &str = "C:\\RacingPoint\\tls";
const CERT_FILENAME: &str = "cert.pem";
const KEY_FILENAME: &str = "key.pem";

/// Load existing PEM files or generate a self-signed cert for the server IP.
///
/// If `cert_path` or `key_path` are None, defaults to `C:\RacingPoint\tls\{cert,key}.pem`.
/// If the PEM files do not exist on disk, generates a new self-signed certificate
/// with IP SAN for `server_ip` and DNS SAN for `localhost`.
pub async fn load_or_generate_rustls_config(
    server_ip: &str,
    cert_path: Option<&str>,
    key_path: Option<&str>,
) -> anyhow::Result<RustlsConfig> {
    let cert_file = cert_path
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("{}\\{}", DEFAULT_CERT_DIR, CERT_FILENAME));
    let key_file = key_path
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("{}\\{}", DEFAULT_CERT_DIR, KEY_FILENAME));

    if !Path::new(&cert_file).exists() || !Path::new(&key_file).exists() {
        tracing::info!("TLS certificates not found, generating self-signed for {}", server_ip);
        generate_and_save(server_ip, &cert_file, &key_file)?;
    }

    let config = RustlsConfig::from_pem_file(&cert_file, &key_file).await?;
    tracing::info!("TLS configured from {} and {}", cert_file, key_file);
    Ok(config)
}

/// Generate a self-signed X.509 certificate with IP SAN and localhost DNS SAN,
/// then write cert.pem and key.pem to the specified paths.
fn generate_and_save(server_ip: &str, cert_path: &str, key_path: &str) -> anyhow::Result<()> {
    // rcgen::CertificateParams::new auto-detects IP addresses from strings,
    // so passing the IP as a string creates a SanType::IpAddress SAN.
    let subject_alt_names = vec![server_ip.to_string(), "localhost".to_string()];
    let CertifiedKey { cert, signing_key } = generate_simple_self_signed(subject_alt_names)?;

    // Ensure parent directories exist
    if let Some(parent) = Path::new(cert_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(parent) = Path::new(key_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(cert_path, cert.pem())?;
    std::fs::write(key_path, signing_key.serialize_pem())?;
    tracing::info!("Self-signed TLS certificate generated for IP {}", server_ip);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Create a unique temp directory for test isolation.
    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("racecontrol_tls_tests")
            .join(name)
            .join(format!("{}", std::process::id()));
        // Clean up any previous run
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn generate_and_save_creates_pem_files() {
        let dir = test_dir("pem_files");
        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");

        generate_and_save(
            "192.168.31.23",
            cert_path.to_str().expect("path"),
            key_path.to_str().expect("path"),
        )
        .expect("generate_and_save should succeed");

        // Both files must exist and be non-empty
        assert!(cert_path.exists(), "cert.pem must exist");
        assert!(key_path.exists(), "key.pem must exist");

        let cert_content = std::fs::read_to_string(&cert_path).expect("read cert");
        let key_content = std::fs::read_to_string(&key_path).expect("read key");

        assert!(!cert_content.is_empty(), "cert.pem must not be empty");
        assert!(!key_content.is_empty(), "key.pem must not be empty");

        // PEM markers
        assert!(
            cert_content.contains("-----BEGIN CERTIFICATE-----"),
            "cert must have PEM header"
        );
        assert!(
            cert_content.contains("-----END CERTIFICATE-----"),
            "cert must have PEM footer"
        );
        assert!(
            key_content.contains("-----BEGIN PRIVATE KEY-----"),
            "key must have PEM header"
        );
        assert!(
            key_content.contains("-----END PRIVATE KEY-----"),
            "key must have PEM footer"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn generate_and_save_cert_has_ip_san() {
        let dir = test_dir("ip_san");
        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");

        generate_and_save(
            "192.168.31.23",
            cert_path.to_str().expect("path"),
            key_path.to_str().expect("path"),
        )
        .expect("generate_and_save should succeed");

        let cert_pem = std::fs::read_to_string(&cert_path).expect("read cert");

        // The cert PEM should be substantial (contains SAN extension data)
        assert!(
            cert_pem.len() > 200,
            "cert PEM should be substantial (has SAN data)"
        );

        // Decode PEM to DER and verify it's a valid certificate structure
        let cert_der = pem_to_der(&cert_pem);
        assert!(!cert_der.is_empty(), "DER-decoded cert must not be empty");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn load_or_generate_creates_missing_files() {
        let dir = test_dir("load_create");
        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");

        // Files should not exist yet
        assert!(!cert_path.exists(), "cert should not exist before call");
        assert!(!key_path.exists(), "key should not exist before call");

        let result = load_or_generate_rustls_config(
            "192.168.31.23",
            Some(cert_path.to_str().expect("path")),
            Some(key_path.to_str().expect("path")),
        )
        .await;

        assert!(
            result.is_ok(),
            "load_or_generate should succeed: {:?}",
            result.err()
        );

        // Files should now exist
        assert!(cert_path.exists(), "cert.pem must be created");
        assert!(key_path.exists(), "key.pem must be created");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn load_or_generate_uses_existing_files() {
        let dir = test_dir("load_existing");
        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");

        // Pre-generate files
        generate_and_save(
            "192.168.31.23",
            cert_path.to_str().expect("path"),
            key_path.to_str().expect("path"),
        )
        .expect("pre-generate");

        // Record file modification times
        let cert_modified = std::fs::metadata(&cert_path)
            .expect("cert metadata")
            .modified()
            .expect("modified time");
        let key_modified = std::fs::metadata(&key_path)
            .expect("key metadata")
            .modified()
            .expect("modified time");

        // Small delay to ensure any regeneration would produce different mtime
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Load again -- should NOT regenerate
        let result = load_or_generate_rustls_config(
            "192.168.31.23",
            Some(cert_path.to_str().expect("path")),
            Some(key_path.to_str().expect("path")),
        )
        .await;

        assert!(result.is_ok(), "load_or_generate should succeed");

        // Files should not have been modified
        let cert_modified_after = std::fs::metadata(&cert_path)
            .expect("cert metadata")
            .modified()
            .expect("modified time");
        let key_modified_after = std::fs::metadata(&key_path)
            .expect("key metadata")
            .modified()
            .expect("modified time");

        assert_eq!(
            cert_modified, cert_modified_after,
            "cert.pem should not be regenerated"
        );
        assert_eq!(
            key_modified, key_modified_after,
            "key.pem should not be regenerated"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Helper: decode PEM to DER bytes (strip header/footer and base64-decode).
    fn pem_to_der(pem: &str) -> Vec<u8> {
        let b64: String = pem
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .collect::<Vec<_>>()
            .join("");
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64)
            .unwrap_or_default()
    }
}
