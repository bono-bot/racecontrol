//! INC-2H — local resume journal tests (MMA G7). Pure; uses a unique temp file per test.

use rc_installer::journal::{InstallJournal, InstallPhase};
use rc_installer::InstallError;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rcinstaller_journal_{}_{}.json",
        std::process::id(),
        name
    ));
    let _ = std::fs::remove_file(&p);
    let _ = std::fs::remove_file(format!("{}.partial", p.display()));
    p
}

#[test]
fn load_missing_is_fresh() {
    let p = tmp("missing");
    let j = InstallJournal::load_or_new(&p).unwrap();
    assert_eq!(j.schema, 1);
    assert!(j.completed.is_empty());
}

#[test]
fn record_is_idempotent() {
    let p = tmp("idempotent");
    let mut j = InstallJournal::new();
    j.record(&p, InstallPhase::Installed).unwrap();
    j.record(&p, InstallPhase::Installed).unwrap();
    assert_eq!(j.completed.len(), 1);
}

#[test]
fn resume_skips_completed() {
    let p = tmp("resume");
    let mut j = InstallJournal::new();
    j.record(&p, InstallPhase::ManifestVerified).unwrap();
    j.record(&p, InstallPhase::Installed).unwrap();
    // a "re-run" reloads from disk and sees what's already done
    let reloaded = InstallJournal::load_or_new(&p).unwrap();
    assert!(reloaded.has(&InstallPhase::Installed));
    assert!(reloaded.has(&InstallPhase::ManifestVerified));
    assert!(!reloaded.has(&InstallPhase::Provisioned));
}

#[test]
fn atomic_write_leaves_no_partial() {
    let p = tmp("atomic");
    let mut j = InstallJournal::new();
    j.record(&p, InstallPhase::BackedUp).unwrap();
    let partial = PathBuf::from(format!("{}.partial", p.display()));
    assert!(!partial.exists(), "stray .partial left behind");
    assert!(p.exists());
    // and the main file parses back
    let _ = InstallJournal::load_or_new(&p).unwrap();
}

#[test]
fn corrupt_journal_is_error_not_panic() {
    let p = tmp("corrupt");
    std::fs::write(&p, b"{ this is not valid json ]").unwrap();
    assert!(matches!(
        InstallJournal::load_or_new(&p),
        Err(InstallError::Journal(_))
    ));
}

#[test]
fn staged_sha_dedup() {
    let p = tmp("sha_dedup");
    let mut j = InstallJournal::new();
    j.record_staged_sha(&p, "abc123").unwrap();
    j.record_staged_sha(&p, "abc123").unwrap();
    j.record_staged_sha(&p, "def456").unwrap();
    assert_eq!(j.staged_sha256.len(), 2);
}
