fn main() {
    // GIT_HASH resolution matches racecontrol/build.rs for consistency across fleet:
    //   1. GIT_HASH_FORCE env var (deploy scripts inject this to guarantee fresh hash)
    //   2. git rev-parse --short HEAD (normal dev builds)
    //   3. "dev" fallback (no git available)
    //
    // Dirty-tree detection: binaries built with uncommitted changes get "-dirty" suffix.
    // Without this, a build after an edit-but-before-commit ships with the PREVIOUS
    // commit's hash baked in, misleading downstream consumers (fleet health, audits).
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

    // VMS PATTERN: asInvoker manifest — deployed as external file.
    // Windows reads `rc-agent.exe.manifest` automatically if it's next to the exe.
    // VMS Connect uses the same requestedExecutionLevel=asInvoker pattern to:
    // 1. Prevent anti-cheat from flagging the process as elevated
    // 2. Ensure consistent behavior regardless of parent process elevation
    // The manifest file is in deploy/ and must be copied to C:\RacingPoint\ alongside the exe.
    println!("cargo:rerun-if-changed=rc-agent.exe.manifest");

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
