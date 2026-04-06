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
    if let Ok(head) = std::fs::read_to_string("../../.git/HEAD") {
        let head = head.trim();
        if let Some(ref_path) = head.strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=../../.git/{ref_path}");
        }
    }
}
