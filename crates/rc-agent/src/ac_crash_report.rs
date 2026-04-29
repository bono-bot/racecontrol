// PACT-20260429-014 AMEND-1 Phase B (RATIFIED-PROCEED-PHASE-B-IMPL d1868d0).
//
// On AC crash, the previous error_message was a generic "Process exited
// unexpectedly (exit code: N)". CSP track-validation failures
// (`require_track`-class crashes that PACT-014 aims to detect) carry their
// real diagnostic text in `Documents\Assetto Corsa\crash_<ts>.report\info.txt`,
// which never reached racecontrol.db. Bono's FP-rate backtest (commit
// `c8e400f`) showed 0/808 hits in 7d on `error_message` — empirical
// confirmation the column was structurally empty for AC crashes.
//
// This module reads the most-recent crash_*.report info.txt (first 512
// chars), PII-redacts user-paths, and returns the snippet so the caller
// can append it to the WS-propagated error_message. Server-side wiring
// already exists: `crates/racecontrol/src/game_launcher_state.rs:144`
// passes `info.error_message` through to `log_game_event`, which writes
// it to `game_launch_events.error_message`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tracing::{debug, warn};

/// First N chars cap per AMEND-1 caveat (~512 char upper bound for info.txt).
pub const MAX_INFO_TXT_CHARS: usize = 512;

/// Locate the most-recent AC crash report and return a PII-redacted
/// snippet of its info.txt content.
///
/// Returns None if:
/// - The user's Documents directory cannot be located (non-Windows or no $USERPROFILE)
/// - `Documents\Assetto Corsa\` does not exist
/// - No `crash_*.report` entries are found
/// - The most-recent entry is unreadable, has no info.txt, or info.txt is empty
///
/// The crash_*.report path is a PK-ZIP archive (confirmed empirically on
/// Pod 6 — see `feedback_pod6_ac_directx_capture_20260429.md` line 47:
/// "PK-ZIP archives containing info.txt, log.txt, errors.txt, race.ini,
/// video.ini, csp.ini, custom_shaders_patch.log, python.ini, py_log.txt,
/// controls.ini, crash_*.dmp"). v1 reads zip archives via the `zip`
/// crate. Earlier Kunos-vanilla layouts may have used directories; that
/// branch remains supported as a fallback for older crash artifacts.
pub fn read_latest_ac_crash_report() -> Option<String> {
    let docs = dirs_next::document_dir()?;
    let ac_dir = docs.join("Assetto Corsa");
    if !ac_dir.is_dir() {
        debug!(path = %ac_dir.display(), "AC documents dir not found — no crash report read");
        return None;
    }

    let latest = find_latest_crash_report(&ac_dir)?;

    let raw = if latest.is_dir() {
        let info_path = latest.join("info.txt");
        match fs::read_to_string(&info_path) {
            Ok(s) if s.is_empty() => {
                debug!(path = %info_path.display(), "AC info.txt (dir) is empty");
                return None;
            }
            Ok(s) => s,
            Err(e) => {
                debug!(path = %info_path.display(), error = %e, "AC info.txt (dir) read failed");
                return None;
            }
        }
    } else {
        match read_info_txt_from_zip(&latest) {
            Some(s) if s.is_empty() => {
                debug!(path = %latest.display(), "AC info.txt (zip) is empty");
                return None;
            }
            Some(s) => s,
            None => return None,
        }
    };

    let truncated: String = raw.chars().take(MAX_INFO_TXT_CHARS).collect();
    Some(redact_user_paths(&truncated))
}

/// Open `crash_<ts>.report` as a PK-ZIP archive and read the contents of
/// the `info.txt` entry. Returns the raw (un-truncated, un-redacted)
/// string so the caller can apply the standard truncation and redaction
/// pipeline used for both layouts.
///
/// All zip-crate errors are swallowed at debug level — Phase B's
/// design-by-contract is "best-effort enrichment, never block the
/// crash-detection path". Returning None means error_message stays at
/// the existing exit-code-based default, which is the same behavior
/// rc-agent has shipped for years; no regression.
fn read_info_txt_from_zip(path: &Path) -> Option<String> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            debug!(path = %path.display(), error = %e, "AC crash report zip open failed");
            return None;
        }
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            debug!(path = %path.display(), error = %e, "AC crash report not a valid zip");
            return None;
        }
    };
    let info_entry = match archive.by_name("info.txt") {
        Ok(e) => e,
        Err(e) => {
            debug!(path = %path.display(), error = %e, "AC crash report has no info.txt entry");
            return None;
        }
    };
    // Bound the read at MAX_INFO_TXT_CHARS * 4 bytes (UTF-8 worst case
    // 4 bytes/char) to avoid runaway memory if the entry header lies
    // about size. Truncation to the char cap happens in the caller after
    // string-conversion, so the byte cap here is a defensive lower bound.
    let cap = MAX_INFO_TXT_CHARS.saturating_mul(4);
    let mut buf = Vec::with_capacity(cap.min(2048));
    if let Err(e) = info_entry.take(cap as u64).read_to_end(&mut buf) {
        debug!(path = %path.display(), error = %e, "AC info.txt zip-entry read failed");
        return None;
    }
    match String::from_utf8(buf) {
        Ok(s) => Some(s),
        Err(e) => {
            // Lossy fallback so we capture *something* on encoding-corrupt
            // entries rather than dropping the whole snippet.
            warn!(
                path = %path.display(),
                error = %e,
                "AC info.txt zip-entry contains invalid UTF-8 — using lossy decode"
            );
            Some(String::from_utf8_lossy(e.as_bytes()).into_owned())
        }
    }
}

/// Find the most-recently-modified entry whose name starts with `crash_`
/// and ends with `.report` inside `ac_dir`. Returns the path or None if
/// no match is found or the directory cannot be enumerated.
fn find_latest_crash_report(ac_dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(ac_dir).ok()?;
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if !name.starts_with("crash_") || !name.ends_with(".report") {
            continue;
        }
        let mtime = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        match &best {
            Some((cur, _)) if *cur >= mtime => {}
            _ => best = Some((mtime, path)),
        }
    }
    best.map(|(_, p)| p)
}

/// Replace `\Users\<name>\` with `\Users\REDACTED\` in arbitrary text.
/// Stack traces and crash dumps frequently embed full home-directory
/// paths (driver names, machine accounts) that we don't want flowing into
/// the central racecontrol.db. This is a coarse pass — info.txt is a
/// 512-char snippet and AC's own logging is the only known producer of
/// `\Users\` paths in this context, so the simpler scan suffices over
/// pulling in `regex`.
fn redact_user_paths(input: &str) -> String {
    let needle = r"\Users\";
    let replacement = r"\Users\REDACTED\";
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find(needle) {
        out.push_str(&rest[..idx]);
        out.push_str(replacement);
        // Skip past `\Users\` and then past the username up to the next `\`
        // (or to end-of-string if no terminating backslash).
        let after = &rest[idx + needle.len()..];
        match after.find('\\') {
            Some(next) => rest = &after[next + 1..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_replaces_username() {
        let input = r"crash at C:\Users\drv_8d1025c4\Documents\Assetto Corsa\log.txt line 42";
        let out = redact_user_paths(input);
        assert_eq!(
            out,
            r"crash at C:\Users\REDACTED\Documents\Assetto Corsa\log.txt line 42"
        );
    }

    #[test]
    fn redact_replaces_multiple_usernames() {
        let input = r"\Users\alice\foo and later \Users\bob\bar";
        let out = redact_user_paths(input);
        assert_eq!(out, r"\Users\REDACTED\foo and later \Users\REDACTED\bar");
    }

    #[test]
    fn redact_handles_trailing_path() {
        // No closing backslash after username — should still redact through end.
        let input = r"\Users\alice";
        let out = redact_user_paths(input);
        assert_eq!(out, r"\Users\REDACTED\");
    }

    #[test]
    fn redact_no_match_passthrough() {
        let input = "no user paths here, just plain text";
        let out = redact_user_paths(input);
        assert_eq!(out, input);
    }

    #[test]
    fn redact_empty_input() {
        assert_eq!(redact_user_paths(""), "");
    }

    /// Build a minimal `crash_<ts>.report` PK-ZIP archive in a temp
    /// directory, then exercise read_info_txt_from_zip end-to-end. Mirrors
    /// the on-disk layout documented at memory line 47 of
    /// `feedback_pod6_ac_directx_capture_20260429.md` — info.txt as a
    /// top-level entry alongside log.txt and dmp.
    #[test]
    fn zip_read_extracts_info_txt() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let tmp = std::env::temp_dir().join(format!(
            "rc-agent-ac-crash-test-{}.report",
            std::process::id()
        ));
        // Defensive cleanup if a prior failed run left a file behind.
        let _ = fs::remove_file(&tmp);

        {
            let f = fs::File::create(&tmp).expect("create temp zip");
            let mut zw = zip::ZipWriter::new(f);
            let opts: SimpleFileOptions = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zw.start_file("info.txt", opts).unwrap();
            zw.write_all(
                br"Can't launch the race: track main layout is damaged. (csp_utils.cpp::require_track:219) at C:\Users\sim6_user\Documents\Assetto Corsa\..."
            ).unwrap();
            zw.start_file("log.txt", opts).unwrap();
            zw.write_all(b"unrelated game log content").unwrap();
            zw.finish().unwrap();
        }

        let snippet = read_info_txt_from_zip(&tmp).expect("zip read returns Some");
        assert!(
            snippet.contains("track main layout is damaged"),
            "expected info.txt content; got: {snippet}"
        );
        // Verify pipeline produces redacted+truncated output too (caller path).
        let truncated: String = snippet.chars().take(MAX_INFO_TXT_CHARS).collect();
        let redacted = redact_user_paths(&truncated);
        assert!(
            !redacted.contains(r"sim6_user"),
            "username should be redacted; got: {redacted}"
        );
        assert!(
            redacted.contains(r"\Users\REDACTED\"),
            "redaction marker should appear; got: {redacted}"
        );

        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn zip_read_returns_none_for_non_zip_file() {
        let tmp = std::env::temp_dir().join(format!(
            "rc-agent-ac-crash-not-a-zip-{}.report",
            std::process::id()
        ));
        let _ = fs::remove_file(&tmp);
        fs::write(&tmp, b"this is not a zip archive, just plain text").unwrap();
        let result = read_info_txt_from_zip(&tmp);
        assert!(result.is_none(), "non-zip file should return None");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn zip_read_returns_none_when_no_info_txt() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let tmp = std::env::temp_dir().join(format!(
            "rc-agent-ac-crash-no-info-{}.report",
            std::process::id()
        ));
        let _ = fs::remove_file(&tmp);

        {
            let f = fs::File::create(&tmp).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts: SimpleFileOptions = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zw.start_file("log.txt", opts).unwrap();
            zw.write_all(b"only log.txt, no info.txt").unwrap();
            zw.finish().unwrap();
        }

        let result = read_info_txt_from_zip(&tmp);
        assert!(result.is_none(), "zip without info.txt should return None");
        let _ = fs::remove_file(&tmp);
    }
}
