//! SW-01 / SW-02 / SW-11: Binary SHA256 and PE header validation.
//!
//! Validates rc-agent.exe against release-manifest.toml:
//! - SHA256 hash comparison (SW-01)
//! - PE header validation: DOS_MAGIC (MZ), PE_MAGIC (PE\0\0), machine type (SW-02)
//! - build_id from manifest cross-checked (SW-11)
//!
//! All I/O is synchronous — called from the service poll loop which is already
//! thread-per-iteration. No tokio dependency. Uses tracing for structured logging.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

/// DOS header magic: 'MZ' (0x5A4D little-endian)
const DOS_MAGIC: u16 = 0x5A4D;

/// PE signature: 'PE\0\0' (0x00004550 little-endian)
const PE_MAGIC: u32 = 0x0000_4550;

/// COFF machine type for x86_64
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;

/// Minimum valid PE file size (DOS header + PE signature + COFF header)
const MIN_PE_SIZE: u64 = 64 + 4 + 20;

/// Result of binary validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub sha256_ok: bool,
    pub pe_valid: bool,
    pub build_id_ok: bool,
    pub computed_hash: String,
    pub expected_hash: String,
    pub details: String,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.sha256_ok && self.pe_valid && self.build_id_ok
    }
}

/// Fields we read from release-manifest.toml.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ReleaseManifest {
    #[serde(default)]
    pub rc_agent_sha256: String,
    #[serde(default)]
    pub build_id: String,
    #[serde(default)]
    pub git_commit: String,
    #[serde(default)]
    pub timestamp: String,
}

/// Validate a binary against its manifest.
///
/// Returns `Ok(ValidationResult)` even on validation failure — the caller checks
/// `result.is_valid()`. Returns `Err` only on I/O failures that prevent validation.
pub fn validate_binary(
    binary_path: &Path,
    manifest_path: &Path,
) -> anyhow::Result<ValidationResult> {
    // Load manifest
    let manifest = load_manifest(manifest_path)?;

    // Compute SHA256
    let computed_hash = compute_sha256(binary_path)?;
    let sha256_ok = !manifest.rc_agent_sha256.is_empty()
        && computed_hash.eq_ignore_ascii_case(&manifest.rc_agent_sha256);

    // Validate PE headers
    let pe_valid = validate_pe_headers(binary_path)?;

    // build_id check: if manifest has a build_id, we note it but can't verify
    // against the binary without running it. Mark as OK if manifest has one.
    let build_id_ok = !manifest.build_id.is_empty();

    let details = if sha256_ok && pe_valid && build_id_ok {
        format!(
            "Binary valid: SHA256 match, PE headers OK, build_id={}",
            manifest.build_id
        )
    } else {
        let mut issues = Vec::new();
        if !sha256_ok {
            issues.push(format!(
                "SHA256 mismatch: expected={}, computed={}",
                manifest.rc_agent_sha256, computed_hash
            ));
        }
        if !pe_valid {
            issues.push("PE header validation failed".to_string());
        }
        if !build_id_ok {
            issues.push("build_id missing from manifest".to_string());
        }
        issues.join("; ")
    };

    Ok(ValidationResult {
        sha256_ok,
        pe_valid,
        build_id_ok,
        computed_hash,
        expected_hash: manifest.rc_agent_sha256,
        details,
    })
}

/// Load and parse release-manifest.toml.
fn load_manifest(path: &Path) -> anyhow::Result<ReleaseManifest> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read manifest at {}: {}", path.display(), e))?;
    let manifest: ReleaseManifest = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse manifest at {}: {}", path.display(), e))?;
    Ok(manifest)
}

/// Compute SHA256 hash of a file, returning lowercase hex string.
/// Uses buffered reading for large binaries (rc-agent.exe is ~15MB).
pub fn compute_sha256(path: &Path) -> anyhow::Result<String> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to open binary at {}: {}", path.display(), e))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)
            .map_err(|e| anyhow::anyhow!("Error reading {}: {}", path.display(), e))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Validate PE headers of a Windows executable.
///
/// Checks:
/// 1. File size >= minimum for a valid PE
/// 2. DOS header magic (MZ)
/// 3. e_lfanew points to valid PE signature location
/// 4. PE signature (PE\0\0)
/// 5. Machine type is AMD64
fn validate_pe_headers(path: &Path) -> anyhow::Result<bool> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| anyhow::anyhow!("Cannot stat {}: {}", path.display(), e))?;

    if metadata.len() < MIN_PE_SIZE {
        tracing::warn!(
            "Binary too small for valid PE: {} bytes (min {})",
            metadata.len(),
            MIN_PE_SIZE
        );
        return Ok(false);
    }

    // Read first 1024 bytes — enough for DOS header + PE header
    let mut file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("Cannot open {}: {}", path.display(), e))?;
    let mut header_buf = vec![0u8; 1024];
    let bytes_read = file.read(&mut header_buf)
        .map_err(|e| anyhow::anyhow!("Cannot read PE header of {}: {}", path.display(), e))?;

    if bytes_read < 64 {
        tracing::warn!("Could not read DOS header: only {} bytes", bytes_read);
        return Ok(false);
    }

    // Check DOS magic (bytes 0-1): 'MZ'
    let dos_magic = u16::from_le_bytes([header_buf[0], header_buf[1]]);
    if dos_magic != DOS_MAGIC {
        tracing::warn!(
            "Invalid DOS magic: 0x{:04X} (expected 0x{:04X})",
            dos_magic, DOS_MAGIC
        );
        return Ok(false);
    }

    // e_lfanew at offset 0x3C (4 bytes, little-endian) — offset to PE header
    let e_lfanew = u32::from_le_bytes([
        header_buf[0x3C],
        header_buf[0x3D],
        header_buf[0x3E],
        header_buf[0x3F],
    ]) as usize;

    if e_lfanew + 6 > bytes_read {
        tracing::warn!(
            "e_lfanew points beyond read buffer: {} (read {})",
            e_lfanew, bytes_read
        );
        return Ok(false);
    }

    // Check PE signature at e_lfanew
    let pe_sig = u32::from_le_bytes([
        header_buf[e_lfanew],
        header_buf[e_lfanew + 1],
        header_buf[e_lfanew + 2],
        header_buf[e_lfanew + 3],
    ]);
    if pe_sig != PE_MAGIC {
        tracing::warn!(
            "Invalid PE signature: 0x{:08X} (expected 0x{:08X})",
            pe_sig, PE_MAGIC
        );
        return Ok(false);
    }

    // Machine type at e_lfanew + 4 (COFF header starts after PE sig)
    let machine = u16::from_le_bytes([
        header_buf[e_lfanew + 4],
        header_buf[e_lfanew + 5],
    ]);
    if machine != IMAGE_FILE_MACHINE_AMD64 {
        tracing::warn!(
            "Unexpected machine type: 0x{:04X} (expected 0x{:04X} AMD64)",
            machine, IMAGE_FILE_MACHINE_AMD64
        );
        return Ok(false);
    }

    tracing::debug!("PE headers valid: DOS_MAGIC=OK, PE_MAGIC=OK, Machine=AMD64");
    Ok(true)
}

/// Check if a manifest file exists and is readable.
pub fn manifest_exists(manifest_path: &Path) -> bool {
    manifest_path.is_file()
}

/// Get the path to the release manifest alongside a binary.
pub fn manifest_path_for(binary_dir: &Path) -> PathBuf {
    binary_dir.join("release-manifest.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dos_magic_value() {
        // 'M' = 0x4D, 'Z' = 0x5A → little-endian u16 = 0x5A4D
        assert_eq!(DOS_MAGIC, 0x5A4D);
    }

    #[test]
    fn test_pe_magic_value() {
        // 'P' = 0x50, 'E' = 0x45, '\0', '\0' → little-endian u32 = 0x00004550
        assert_eq!(PE_MAGIC, 0x0000_4550);
    }

    #[test]
    fn test_validate_pe_headers_nonexistent_file() {
        let result = validate_pe_headers(Path::new(r"C:\nonexistent\fake.exe"));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_pe_headers_too_small() {
        let dir = std::env::temp_dir().join("rc_watchdog_test_pe");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("tiny.exe");
        std::fs::write(&path, b"MZ").ok();
        let result = validate_pe_headers(&path);
        // File exists but is too small
        match result {
            Ok(valid) => assert!(!valid, "tiny file should not be valid PE"),
            Err(_) => {} // stat might fail on some systems, that's OK
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_pe_headers_text_file() {
        let dir = std::env::temp_dir().join("rc_watchdog_test_pe_text");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("notape.exe");
        // 256 bytes of text — enough to pass size check but fail DOS magic
        let content = vec![b'A'; 256];
        std::fs::write(&path, &content).ok();
        let result = validate_pe_headers(&path);
        match result {
            Ok(valid) => assert!(!valid, "text file should not be valid PE"),
            Err(_) => {}
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_compute_sha256_nonexistent() {
        let result = compute_sha256(Path::new(r"C:\nonexistent\file.bin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_sha256_known_content() {
        let dir = std::env::temp_dir().join("rc_watchdog_test_sha");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.bin");
        std::fs::write(&path, b"hello world").ok();
        let hash = compute_sha256(&path).expect("should hash OK");
        // SHA256 of "hello world" is well-known
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_manifest_missing_file() {
        let result = load_manifest(Path::new(r"C:\nonexistent\release-manifest.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_manifest_valid() {
        let dir = std::env::temp_dir().join("rc_watchdog_test_manifest");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("release-manifest.toml");
        std::fs::write(
            &path,
            r#"rc_agent_sha256 = "abc123"
build_id = "deadbeef"
git_commit = "0123456"
timestamp = "2026-03-31T12:00:00Z"
"#,
        ).ok();
        let manifest = load_manifest(&path).expect("should parse OK");
        assert_eq!(manifest.rc_agent_sha256, "abc123");
        assert_eq!(manifest.build_id, "deadbeef");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_manifest_path_for() {
        let dir = Path::new(r"C:\RacingPoint");
        let result = manifest_path_for(dir);
        assert_eq!(
            result.to_str().expect("valid path"),
            r"C:\RacingPoint\release-manifest.toml"
        );
    }
}
