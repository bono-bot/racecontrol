//! spec §11 compliance — D-INSTALLER-1 (single user-visible installer).
//!
//! Manifest JSON reconciled to the canonical contract `release.ts` (2026-05-31):
//! ReleaseArtifact carries `artifact_id` (not `download_url`); `cut_at` is a
//! number (Unix-ms); `release_ring` ∈ {canary, early, general}; `artifacts`
//! is non-empty.

use rc_installer::{Profile, ReleaseManifest};

#[test]
fn profile_is_server_or_pod_only_no_flavor() {
    // The only operator-facing decision is Server vs Pod. There is no
    // online/offline/stub/flavor variant in the Profile type.
    let all = [Profile::Server, Profile::Pod];
    assert_eq!(all.len(), 2);
    for p in all {
        let s = p.as_str();
        for banned in ["online", "offline", "flavor", "stub", "isolated"] {
            assert!(!s.contains(banned), "profile label '{s}' must not contain '{banned}'");
        }
    }
}

#[test]
fn single_installer_artifact_per_manifest() {
    let json = r#"{
      "release_id":"rel-0001","release_class":"feature","release_ring":"general",
      "artifacts":[{"artifact_id":"a1","sha256":"00","size_bytes":1,"target":"rc-agent"}],
      "installer_artifact":{"sha256":"00","size_bytes":1,"download_url":"https://console.racecontrol.in/install/bootstrapper","signing_key_id":"k"},
      "previous_release_id":null,"cut_at":1,"signing_key_id":"k","signature":"x"
    }"#;
    let m = ReleaseManifest::from_json(json).unwrap();
    assert_eq!(m.installer_artifact_count(), 1);
}
