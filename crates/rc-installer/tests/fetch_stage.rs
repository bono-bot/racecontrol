//! INC-3 contract test: fetch→verify→stage over a MOCK transport (no network).
//!
//! Proves the load-bearing invariants:
//!   - `fetch_manifest` returns Ok ONLY after the signature verifies (a
//!     placeholder/tampered manifest fails closed, nothing returned).
//!   - `fetch_and_stage_artifact` verifies sha256 IN MEMORY before any write:
//!     a tampered payload leaves NOTHING on disk (not even `.partial`).
//!   - a good artifact is staged atomically at `<dest>/<target>` with no
//!     leftover `.partial`.

use std::collections::HashMap;
use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};
use rc_installer::artifact_stager::{artifact_url, fetch_and_stage_artifact, stage_all_artifacts};
use rc_installer::error::VerifyError;
use rc_installer::manifest::{InstallerArtifact, ReleaseArtifact, ReleaseManifest};
use rc_installer::manifest_fetcher::{
    check_installer_download_url, fetch_manifest, manifest_url, FetchError, Fetcher,
};
use rc_installer::trusted_keys::{TrustedKey, TrustedKeySet};
use sha2::{Digest, Sha256};

const KID: &str = "rc-release-test-001";
const BASE: &str = "https://console.racecontrol.in";
const AGENT_BYTES: &[u8] = b"rc-agent-payload";

fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// A signed manifest + trust set + the artifact id, mirroring
/// `manifest_signature.rs`'s deterministic fixture.
fn valid_fixture() -> (ReleaseManifest, TrustedKeySet) {
    let sk = SigningKey::from_bytes(&[7u8; 32]);
    let installer_artifact = InstallerArtifact {
        sha256: sha256_hex(b"installer-binary-bytes"),
        size_bytes: 21,
        download_url: "https://console.racecontrol.in/install/bootstrapper".into(),
        signing_key_id: KID.into(),
    };
    let artifacts = vec![ReleaseArtifact {
        artifact_id: "rc-agent-2026Q2-001".into(),
        sha256: sha256_hex(AGENT_BYTES),
        size_bytes: AGENT_BYTES.len() as u64,
        target: "rc-agent".into(),
    }];
    let mut manifest = ReleaseManifest {
        release_id: "rel-2026Q2-001".into(),
        release_class: "feature".into(),
        release_ring: "general".into(),
        artifacts,
        installer_artifact,
        previous_release_id: None,
        cut_at: 1_748_649_600_000,
        signing_key_id: KID.into(),
        signature: String::new(),
    };
    let bytes = manifest.canonical_signed_bytes().unwrap();
    manifest.signature = hex::encode(sk.sign(&bytes).to_bytes());
    let keys = TrustedKeySet {
        keys: vec![TrustedKey {
            kid: KID.into(),
            public_key: hex::encode(sk.verifying_key().to_bytes()),
            status: "active".into(),
        }],
    };
    (manifest, keys)
}

/// A canned-response mock: URL → (status, bytes). Unknown URL → transport error.
struct MockFetcher {
    routes: HashMap<String, (u16, Vec<u8>)>,
}
impl Fetcher for MockFetcher {
    fn get(&self, url: &str) -> Result<(u16, Vec<u8>), String> {
        self.routes
            .get(url)
            .cloned()
            .ok_or_else(|| format!("no mock route for {url}"))
    }
}

fn mock_serving(manifest: &ReleaseManifest, agent_bytes: &[u8]) -> MockFetcher {
    let mut routes = HashMap::new();
    routes.insert(
        manifest_url(BASE),
        (200, serde_json::to_vec(manifest).unwrap()),
    );
    routes.insert(
        artifact_url(BASE, &manifest.artifacts[0].artifact_id),
        (200, agent_bytes.to_vec()),
    );
    MockFetcher { routes }
}

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    // Per-test unique dir under the cargo target tmp area (no Date/rand needed).
    let dir = std::env::temp_dir().join(format!("rc-installer-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn fetch_manifest_returns_only_after_verify() {
    let (manifest, trusted) = valid_fixture();
    let fetcher = mock_serving(&manifest, AGENT_BYTES);
    let got = fetch_manifest(&fetcher, BASE, &trusted).expect("verified manifest");
    assert_eq!(got.release_id, "rel-2026Q2-001");
}

#[test]
fn fetch_manifest_fails_closed_on_untrusted_key() {
    let (manifest, _) = valid_fixture();
    let fetcher = mock_serving(&manifest, AGENT_BYTES);
    let empty = TrustedKeySet { keys: vec![] };
    match fetch_manifest(&fetcher, BASE, &empty) {
        Err(FetchError::Verify(VerifyError::UnknownKid(_))) => {}
        other => panic!("expected fail-closed UnknownKid, got {other:?}"),
    }
}

#[test]
fn fetch_manifest_maps_non_2xx_to_http() {
    let (manifest, trusted) = valid_fixture();
    let mut fetcher = mock_serving(&manifest, AGENT_BYTES);
    fetcher.routes.insert(manifest_url(BASE), (503, b"down".to_vec()));
    assert_eq!(
        fetch_manifest(&fetcher, BASE, &trusted),
        Err(FetchError::Http { status: 503 })
    );
}

#[test]
fn good_artifact_is_staged_atomically() {
    let (manifest, _) = valid_fixture();
    let fetcher = mock_serving(&manifest, AGENT_BYTES);
    let dest = tmp_dir("stage-ok");

    let staged =
        fetch_and_stage_artifact(&fetcher, BASE, &manifest.artifacts[0], &dest).expect("staged");

    assert_eq!(staged, dest.join("rc-agent"));
    assert_eq!(std::fs::read(&staged).unwrap(), AGENT_BYTES);
    // No leftover .partial after the atomic rename.
    assert!(!dest.join("rc-agent.partial").exists());

    let _ = std::fs::remove_dir_all(&dest);
}

#[test]
fn tampered_artifact_stages_nothing() {
    let (manifest, _) = valid_fixture();
    // Serve WRONG bytes for the artifact (its sha won't match the signed entry).
    let fetcher = mock_serving(&manifest, b"TAMPERED-payload");
    let dest = tmp_dir("stage-tampered");

    let result = fetch_and_stage_artifact(&fetcher, BASE, &manifest.artifacts[0], &dest);
    match result {
        Err(FetchError::Artifact(VerifyError::ArtifactShaMismatch { target })) => {
            assert_eq!(target, "rc-agent");
        }
        other => panic!("expected ArtifactShaMismatch, got {other:?}"),
    }
    // TOCTOU: nothing on disk — not the final file, not a .partial.
    assert!(!dest.join("rc-agent").exists(), "no final file");
    assert!(!dest.join("rc-agent.partial").exists(), "no partial file");

    let _ = std::fs::remove_dir_all(&dest);
}

#[test]
fn stage_all_returns_paths_in_order() {
    let (manifest, _) = valid_fixture();
    let fetcher = mock_serving(&manifest, AGENT_BYTES);
    let dest = tmp_dir("stage-all");

    let staged = stage_all_artifacts(&fetcher, BASE, &manifest, &dest).expect("all staged");
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0], dest.join("rc-agent"));

    let _ = std::fs::remove_dir_all(&dest);
}

#[test]
fn installer_download_url_prefix_is_enforced() {
    let (good, _) = valid_fixture();
    assert_eq!(check_installer_download_url(&good), Ok(()));

    let mut bad = good;
    bad.installer_artifact.download_url = "https://evil.example/install/bootstrapper".into();
    match check_installer_download_url(&bad) {
        Err(FetchError::UrlPrefix(u)) => assert!(u.starts_with("https://evil.example")),
        other => panic!("expected UrlPrefix, got {other:?}"),
    }
}

#[test]
fn url_builders_stay_under_install_path() {
    assert_eq!(
        manifest_url(BASE),
        "https://console.racecontrol.in/install/release-manifest.json"
    );
    assert_eq!(
        artifact_url(BASE, "rc-agent-2026Q2-001"),
        "https://console.racecontrol.in/install/rc-agent-2026Q2-001"
    );
    // Trailing slash on the base must not double up.
    assert_eq!(
        manifest_url("https://console.racecontrol.in/"),
        "https://console.racecontrol.in/install/release-manifest.json"
    );
    // Sanity: the dest_dir helper is exercised through staging above; touch Path
    // here so an unused-import lint never masks a real regression.
    let _ = Path::new("/");
}
