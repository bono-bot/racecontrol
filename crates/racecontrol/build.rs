fn main() {
    // GIT_HASH resolution priority:
    //   1. GIT_HASH_FORCE env var (deploy scripts inject this to guarantee fresh hash)
    //   2. git rev-parse --short HEAD (normal dev builds)
    //   3. "dev" fallback (no git available)
    //
    // Dirty-tree detection: if the working tree has uncommitted changes at build
    // time, the hash is marked with "-dirty". This catches the case where a
    // developer edits files, builds, then commits — the binary would otherwise
    // ship with the PREVIOUS commit's hash baked in, misleading every downstream
    // consumer (fleet health, deploy-audit, forensics).
    //
    // Audit trail: commit f162d8a4 was deployed with build_id "e6a714e0" because
    // the cargo build ran BEFORE the commit landed. Dirty-tree marking catches
    // this at build time instead of deploy time.

    let forced = std::env::var("GIT_HASH_FORCE").ok();
    let hash = forced
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "dev".to_string());

    // Check for uncommitted changes (dirty working tree).
    // Ignore untracked files — only tracked files that differ from HEAD matter
    // for binary correctness. Use `git diff --quiet HEAD` which exits 1 if dirty.
    let dirty = std::process::Command::new("git")
        .args(["diff", "--quiet", "HEAD", "--"])
        .status()
        .ok()
        .map(|s| !s.success())
        .unwrap_or(false);

    let final_hash = if dirty && hash != "dev" {
        format!("{hash}-dirty")
    } else {
        hash
    };

    println!("cargo:rustc-env=GIT_HASH={final_hash}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GIT_HASH_FORCE");

    // Watch .git/HEAD (detects branch switches) AND the actual ref file
    // (detects new commits on the current branch). Without the ref file,
    // cargo caches the old GIT_HASH across commits on the same branch.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/packed-refs");
    if let Ok(head) = std::fs::read_to_string("../../.git/HEAD") {
        let head = head.trim();
        if let Some(ref_path) = head.strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=../../.git/{ref_path}");
        }
    }
}
