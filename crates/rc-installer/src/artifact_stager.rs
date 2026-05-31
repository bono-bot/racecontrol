//! INC-3b — fetch each release artifact, VERIFY its sha256 IN MEMORY before it
//! touches disk, then stage it atomically.
//!
//! TOCTOU-safety (spec §4.2 / §7 step 7): the bytes are hashed and checked
//! against the signed manifest entry BEFORE any write, so a tampered payload is
//! never persisted — not even transiently to a `.partial`. Staging then writes
//! `<dest>/<target>.partial` and atomically renames to `<dest>/<target>`, so a
//! crash mid-write never leaves a half-file at the real path (resume-safe).
//!
//! Pure over the [`Fetcher`] seam + an injected destination dir, so the
//! verify-before-disk invariant is unit-tested on Linux (tempdir, mock fetcher)
//! with no network. There is NO raw-fetch path that skips verification.

use std::fs;
use std::path::{Path, PathBuf};

use crate::manifest::{ReleaseArtifact, ReleaseManifest};
use crate::manifest_fetcher::{FetchError, Fetcher};
use crate::signature_verifier::verify_artifact_sha;

/// The download URL for one artifact, derived from the tier-1 base + its
/// `artifact_id` (the manifest carries `artifact_id`, not a per-artifact URL —
/// the CDN path is `{base}/install/{artifact_id}`).
pub fn artifact_url(base: &str, artifact_id: &str) -> String {
    format!("{}/install/{}", base.trim_end_matches('/'), artifact_id)
}

/// Where a staged artifact lands. `target` is the manifest's logical name
/// (e.g. `rc-agent`); kept as a leaf filename under `dest_dir`.
fn staged_path(dest_dir: &Path, target: &str) -> PathBuf {
    dest_dir.join(target)
}

/// Fetch ONE artifact, verify its sha256 in memory, and stage it atomically.
/// Returns the final staged path. Nothing is written unless verification passes.
pub fn fetch_and_stage_artifact(
    fetcher: &dyn Fetcher,
    base: &str,
    artifact: &ReleaseArtifact,
    dest_dir: &Path,
) -> Result<PathBuf, FetchError> {
    let url = artifact_url(base, &artifact.artifact_id);
    let (status, bytes) = fetcher.get(&url).map_err(FetchError::Transport)?;
    if !(200..300).contains(&status) {
        return Err(FetchError::Http { status });
    }

    // GATE — hash in memory and compare to the SIGNED manifest entry BEFORE any
    // disk write. A mismatch returns here; nothing is staged (not even .partial).
    verify_artifact_sha(artifact, &bytes).map_err(FetchError::Artifact)?;

    fs::create_dir_all(dest_dir).map_err(|e| FetchError::Io(e.to_string()))?;
    let final_path = staged_path(dest_dir, &artifact.target);
    let partial_path = dest_dir.join(format!("{}.partial", artifact.target));

    // Write to .partial, then atomically rename. A crash mid-write leaves only
    // a .partial (ignored on resume), never a truncated file at the real path.
    fs::write(&partial_path, &bytes).map_err(|e| FetchError::Io(e.to_string()))?;
    fs::rename(&partial_path, &final_path).map_err(|e| FetchError::Io(e.to_string()))?;
    Ok(final_path)
}

/// Fetch + verify + stage EVERY artifact in a (already-verified) manifest.
/// Returns the staged paths in manifest order. Fails closed on the first bad
/// artifact — a later good artifact never masks an earlier tampered one.
pub fn stage_all_artifacts(
    fetcher: &dyn Fetcher,
    base: &str,
    manifest: &ReleaseManifest,
    dest_dir: &Path,
) -> Result<Vec<PathBuf>, FetchError> {
    let mut staged = Vec::with_capacity(manifest.artifacts.len());
    for artifact in &manifest.artifacts {
        staged.push(fetch_and_stage_artifact(fetcher, base, artifact, dest_dir)?);
    }
    Ok(staged)
}
