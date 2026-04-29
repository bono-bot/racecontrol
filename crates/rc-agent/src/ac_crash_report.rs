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
use std::path::PathBuf;
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
/// The crash_*.report path may be either a directory (Kunos-vanilla layout)
/// or a zip file (CSP-era layout). v0 of this reader handles the directory
/// case. If the most-recent entry is a regular file (zip), this returns
/// None and emits a warn-level trace. Adding zip support requires the `zip`
/// crate dependency — left for v1 once we observe the on-disk format on
/// real Pod 6 crash artifacts.
pub fn read_latest_ac_crash_report() -> Option<String> {
    let docs = dirs_next::document_dir()?;
    let ac_dir = docs.join("Assetto Corsa");
    if !ac_dir.is_dir() {
        debug!(path = %ac_dir.display(), "AC documents dir not found — no crash report read");
        return None;
    }

    let latest = find_latest_crash_report(&ac_dir)?;

    let info_path = if latest.is_dir() {
        latest.join("info.txt")
    } else {
        warn!(
            path = %latest.display(),
            "AC crash report is a file (likely zip) — v0 reader handles directories only; \
             zip support deferred to v1 (would require `zip` crate dep). Returning None."
        );
        return None;
    };

    let raw = match fs::read_to_string(&info_path) {
        Ok(s) if s.is_empty() => {
            debug!(path = %info_path.display(), "AC info.txt is empty");
            return None;
        }
        Ok(s) => s,
        Err(e) => {
            debug!(path = %info_path.display(), error = %e, "AC info.txt read failed");
            return None;
        }
    };

    let truncated: String = raw.chars().take(MAX_INFO_TXT_CHARS).collect();
    Some(redact_user_paths(&truncated))
}

/// Find the most-recently-modified entry whose name starts with `crash_`
/// and ends with `.report` inside `ac_dir`. Returns the path or None if
/// no match is found or the directory cannot be enumerated.
fn find_latest_crash_report(ac_dir: &PathBuf) -> Option<PathBuf> {
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
}
