fn main() {
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "dev".to_string());

    println!("cargo:rustc-env=GIT_HASH={hash}");

    // Force rebuild on every cargo invocation — GIT_HASH must always be fresh.
    // Previous approach (watching .git/HEAD + ref file) was unreliable because
    // cargo's mtime-based detection misses fast-forward pulls and parallel session
    // commits. The cost is ~0.1s per build to re-run git rev-parse.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GIT_HASH_FORCE");
    // Always rerun: write a timestamp to guarantee cargo sees a change
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    if let Ok(head) = std::fs::read_to_string("../../.git/HEAD") {
        let head = head.trim();
        if let Some(ref_path) = head.strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=../../.git/{ref_path}");
        }
    }
}
