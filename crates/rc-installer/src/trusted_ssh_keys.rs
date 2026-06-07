//! Embedded SSH authorized-keys set + idempotent reconcile (MMA-DIAGNOSE M4,
//! kills RC#3 "embedded key non-revocable").
//!
//! Mirrors [`crate::trusted_keys`] (the manifest-signing key set) but for the
//! **pod login** key(s) written to `C:\ProgramData\ssh\administrators_authorized_keys`
//! on `own` venues. The set is shipped in the binary; `reconcile` computes the new
//! file content so that re-provisioning ROTATES/REVOKES rather than only appending:
//!
//! - `active`      keys are ensured present.
//! - `revoked` / `placeholder` keys are REMOVED if present (revocation propagates
//!   on the next install — no rebuild-to-append-then-never-remove footgun).
//! - **foreign keys** (operator's own, or anything not in our set) are NEVER touched.
//!
//! Pre-key-ceremony posture (IH-6 / M3): the control-node key ships as
//! `Placeholder`, so a stock binary provisions the sshd surface but installs NO
//! login key until a deliberate ceremony flips it to `Active` and rebuilds.

/// Lifecycle status of an embedded SSH key. Lowercase string form matches
/// [`crate::trusted_keys::TrustedKey::status`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshKeyStatus {
    /// Ensure present in authorized_keys.
    Active,
    /// Pre-ceremony / not-yet-trusted — treated as "must be absent".
    Placeholder,
    /// Compromised / retired — must be removed from authorized_keys.
    Revoked,
}

/// One embedded login key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustedSshKey {
    pub kid: &'static str,
    /// Full single-line `authorized_keys` entry (`<type> <base64> [comment]`),
    /// no per-key options (so field-1 is the base64 blob).
    pub line: &'static str,
    pub status: SshKeyStatus,
}

impl TrustedSshKey {
    /// The base64 key material — the stable identity used for matching, robust to
    /// comment changes and to lines that carry option prefixes.
    pub fn blob(&self) -> &'static str {
        blob_of(self.line).unwrap_or(self.line)
    }
}

/// The set shipped in the binary.
#[derive(Debug, Clone, Copy)]
pub struct TrustedSshKeySet {
    pub keys: &'static [TrustedSshKey],
}

/// Outcome of reconciling the embedded set against current file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciliation {
    /// New file content (LF-joined, no trailing blank-line churn).
    pub content: String,
    /// Lines added (active keys that were absent).
    pub added: Vec<String>,
    /// Lines removed (managed keys that are revoked/placeholder).
    pub removed: Vec<String>,
    /// `true` when content differs from input → caller must write.
    pub changed: bool,
}

/// Extract the base64 blob (the long token after the key-type) from an
/// authorized_keys line. Returns the first token that looks like a key type's
/// payload: we take the token following a recognized key-type token.
fn blob_of(line: &str) -> Option<&str> {
    let toks: Vec<&str> = line.split_whitespace().collect();
    for (i, t) in toks.iter().enumerate() {
        let is_type = t.starts_with("ssh-")
            || t.starts_with("ecdsa-")
            || t.starts_with("sk-")
            || *t == "ssh-ed25519";
        if is_type {
            if let Some(b) = toks.get(i + 1) {
                return Some(b);
            }
        }
    }
    None
}

/// The control-node (HQ) login identity. PUBLIC key — safe to ship; grants nothing
/// without the private half. Ships `Placeholder` until the key ceremony
/// (Captain-verified fingerprint out-of-band) flips it to `Active` + rebuilds.
const CONTROL_NODE_KEY: TrustedSshKey = TrustedSshKey {
    kid: "control-node-bono-2026-06",
    line: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINXHsKePP/FWYehyBgIFNzzaT3dAQ4o9QgDFsoTCfj/a bono@racingpoint.in",
    status: SshKeyStatus::Placeholder,
};

const SHIPPED_KEYS: [TrustedSshKey; 1] = [CONTROL_NODE_KEY];

/// The embedded login-key set shipped in this binary.
pub fn embedded() -> TrustedSshKeySet {
    TrustedSshKeySet { keys: &SHIPPED_KEYS }
}

impl TrustedSshKeySet {
    /// Compute the reconciled `authorized_keys` content. Idempotent: applying the
    /// result again yields no further `added`/`removed`.
    pub fn reconcile(&self, current: &str) -> Reconciliation {
        let active_blobs: Vec<&str> = self
            .keys
            .iter()
            .filter(|k| k.status == SshKeyStatus::Active)
            .map(|k| k.blob())
            .collect();
        // Managed-but-not-active (revoked/placeholder) → must be removed if present.
        let drop_blobs: Vec<&str> = self
            .keys
            .iter()
            .filter(|k| k.status != SshKeyStatus::Active)
            .map(|k| k.blob())
            .collect();

        let mut kept: Vec<String> = Vec::new();
        let mut removed: Vec<String> = Vec::new();
        let mut present_active: Vec<&str> = Vec::new();

        for raw in current.lines() {
            let line = raw.trim_end();
            let trimmed = line.trim_start();
            // Preserve blanks + comments verbatim.
            if trimmed.is_empty() || trimmed.starts_with('#') {
                kept.push(line.to_string());
                continue;
            }
            match blob_of(line) {
                // A managed key scheduled for removal.
                Some(b) if drop_blobs.contains(&b) => removed.push(line.to_string()),
                // A managed active key already present — keep, remember (dedup adds).
                Some(b) if active_blobs.contains(&b) => {
                    present_active.push(b);
                    kept.push(line.to_string());
                }
                // Foreign / operator key — NEVER touch.
                _ => kept.push(line.to_string()),
            }
        }

        let mut added: Vec<String> = Vec::new();
        for k in self.keys.iter().filter(|k| k.status == SshKeyStatus::Active) {
            if !present_active.contains(&k.blob()) {
                added.push(k.line.to_string());
            }
        }

        let mut out_lines = kept.clone();
        out_lines.extend(added.clone());
        // Trim trailing empties, then ensure a single trailing newline if non-empty.
        while matches!(out_lines.last(), Some(l) if l.trim().is_empty()) {
            out_lines.pop();
        }
        let content = if out_lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", out_lines.join("\n"))
        };
        let changed = content != normalize(current);
        Reconciliation { content, added, removed, changed }
    }
}

/// Normalize input for an honest `changed` comparison (LF, single trailing NL).
fn normalize(s: &str) -> String {
    let mut lines: Vec<&str> = s.lines().map(|l| l.trim_end()).collect();
    while matches!(lines.last(), Some(l) if l.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() { String::new() } else { format!("{}\n", lines.join("\n")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTIVE: &str = "ssh-ed25519 AAAAACTIVEacme active@hq";
    const REVOKED: &str = "ssh-ed25519 AAAAREVOKEDxxx old@hq";
    const FOREIGN: &str = "ssh-ed25519 AAAAFOREIGNooo operator@venue";

    fn set(keys: &'static [TrustedSshKey]) -> TrustedSshKeySet {
        TrustedSshKeySet { keys }
    }

    static ONE_ACTIVE: [TrustedSshKey; 1] = [TrustedSshKey {
        kid: "k-active",
        line: ACTIVE,
        status: SshKeyStatus::Active,
    }];
    static ACTIVE_AND_REVOKED: [TrustedSshKey; 2] = [
        TrustedSshKey { kid: "k-active", line: ACTIVE, status: SshKeyStatus::Active },
        TrustedSshKey { kid: "k-old", line: REVOKED, status: SshKeyStatus::Revoked },
    ];

    #[test]
    fn adds_active_key_to_empty_file() {
        let r = set(&ONE_ACTIVE).reconcile("");
        assert_eq!(r.added, vec![ACTIVE.to_string()]);
        assert!(r.removed.is_empty());
        assert!(r.changed);
        assert_eq!(r.content, format!("{ACTIVE}\n"));
    }

    #[test]
    fn idempotent_second_run_no_change() {
        let s = set(&ONE_ACTIVE);
        let first = s.reconcile("");
        let second = s.reconcile(&first.content);
        assert!(second.added.is_empty() && second.removed.is_empty());
        assert!(!second.changed);
        assert_eq!(second.content, first.content);
    }

    #[test]
    fn never_touches_foreign_operator_keys() {
        let r = set(&ONE_ACTIVE).reconcile(FOREIGN);
        assert!(r.content.contains(FOREIGN), "operator key must survive");
        assert!(r.content.contains(ACTIVE), "active key still added");
        assert!(r.removed.is_empty());
    }

    #[test]
    fn revocation_removes_managed_key_keeps_foreign() {
        // File has the revoked key + a foreign key; active key absent.
        let current = format!("{REVOKED}\n{FOREIGN}\n");
        let r = set(&ACTIVE_AND_REVOKED).reconcile(&current);
        assert_eq!(r.removed, vec![REVOKED.to_string()]);
        assert!(!r.content.contains(REVOKED), "revoked key must be gone");
        assert!(r.content.contains(FOREIGN), "foreign key preserved");
        assert!(r.content.contains(ACTIVE), "active key added");
    }

    #[test]
    fn placeholder_control_node_key_installs_nothing() {
        // The SHIPPED set (control-node = Placeholder) must not install a login key.
        let r = embedded().reconcile("");
        assert!(r.added.is_empty(), "placeholder key must NOT be added pre-ceremony");
        assert!(r.content.is_empty());
    }

    #[test]
    fn placeholder_key_present_is_removed() {
        // If a stale placeholder key somehow sits in the file, reconcile removes it.
        let line = embedded().keys[0].line;
        let r = embedded().reconcile(&format!("{line}\n{FOREIGN}\n"));
        assert!(r.removed.contains(&line.to_string()));
        assert!(r.content.contains(FOREIGN));
    }

    #[test]
    fn comments_and_blanks_preserved() {
        let current = "# managed by rc-installer\n\n";
        let r = set(&ONE_ACTIVE).reconcile(current);
        assert!(r.content.contains("# managed by rc-installer"));
        assert!(r.content.contains(ACTIVE));
    }
}
