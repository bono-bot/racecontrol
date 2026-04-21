//! Phase 445 D-14 safety gate — structural enum-tagging audit.
//!
//! Purpose: FAIL at `cargo test` time if any rc-common struct/enum receives
//! `#[derive(TS)]` (or `#[cfg_attr(feature = "ts-rs", derive(TS))]`) while ALSO
//! carrying `#[serde(tag = ..., content = ...)]` / `#[serde(tag = ...)]`
//! (adjacently or internally tagged) OR owning a field with `#[serde(flatten)]`.
//!
//! Why this exists (see .planning/phases/445-.../445-RESEARCH.md § "Adjacently-tagged enums"):
//! ts-rs + serde-compat emits a DIFFERENT TypeScript shape for adjacently-tagged
//! enums than hand-written types. Silent drift on a WS-adjacent type would
//! break admin's Zod validators at runtime with zero compile-time warning.
//!
//! Current baseline: zero TS derives exist in rc-common. This test passes
//! trivially. Its value accrues when Plan 02a adds its first `#[derive(TS)]` —
//! a PR that accidentally annotates an adjacently-tagged or flatten-owning
//! type will fail this test BEFORE hitting CI's drift check.
//!
//! Related files (D-14 SKIP-list — 10 sites, authoritative):
//! - crates/rc-common/src/protocol.rs:141 (ServerMessage, tag+content)
//! - crates/rc-common/src/protocol.rs:772 (AgentMessage, tag+content)
//! - crates/rc-common/src/protocol.rs:1173 (DashboardEvent, tag+content)
//! - crates/rc-common/src/protocol.rs:1543 (AiChannelMessage, tag+content)
//! - crates/rc-common/src/protocol.rs:1584 (ClientAction, tag+content)
//! - crates/rc-common/src/protocol.rs:1633 (DashboardCommand, tag+content)
//! - crates/rc-common/src/protocol.rs:755 (flatten field)
//! - crates/rc-common/src/types.rs:871 (GameLaunchInfo, tag+content)
//! - crates/rc-common/src/types.rs:1114 (flatten field)
//! - crates/rc-common/src/mesh_types.rs:196 (MeshMessage, internally tagged)

use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// Walk `dir` recursively and collect every `.rs` file path.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(collect_rs_files(&path));
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    out
}

/// Given a byte offset within `content`, return (line_number, column) both 1-indexed.
fn offset_to_line_col(content: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in content.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Given a byte offset of the start of a TS-derive attribute, find the next
/// `struct` / `enum` definition and the byte range of its block body (inclusive
/// of the opening `{` and closing `}` at brace-depth 0).
///
/// Returns Some((body_start, body_end)) if a well-formed block is found, else None.
fn find_block_after(content: &str, derive_offset: usize) -> Option<(usize, usize)> {
    let tail = &content[derive_offset..];
    // Find the first `{` after the derive — that's the opening brace of the
    // struct/enum block. We intentionally do NOT try to parse tokens; the
    // simple brace-depth tracker below handles nested blocks.
    let open_rel = tail.find('{')?;
    let open_abs = derive_offset + open_rel;
    let after_open = &content[open_abs + 1..];
    let mut depth: i32 = 1;
    let mut i = 0usize;
    let bytes = after_open.as_bytes();
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((open_abs, open_abs + 1 + i));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Given a byte offset where a TS-derive attribute starts, find the FULL
/// attribute cluster — the contiguous run of `#[...]` attribute lines BEFORE
/// and AFTER the derive, stopping at the first non-attribute non-comment line
/// (which is typically the `pub struct`/`pub enum` header itself, or a blank
/// line separating clusters).
///
/// We need BOTH directions because:
/// - Typical ordering is `#[serde(...)]` above, `#[derive(...)]` below.
/// - But `#[cfg_attr(..., derive(TS))]` can appear above the serde attribute
///   too (less common but valid).
/// - Doc comments (`///` / `//!`) are allowed inside the cluster.
///
/// Returns `(cluster_start, cluster_end)` as byte offsets.
fn find_attribute_cluster(content: &str, derive_offset: usize) -> (usize, usize) {
    let bytes = content.as_bytes();

    // Find the start of the derive's line.
    let mut derive_line_start = derive_offset;
    while derive_line_start > 0 && bytes[derive_line_start - 1] != b'\n' {
        derive_line_start -= 1;
    }

    // --- Walk BACKWARDS to find cluster_start ---
    let mut cluster_start = derive_line_start;
    while cluster_start > 0 {
        // Find the previous line.
        let prev_nl = cluster_start - 1; // The '\n' at end of previous line.
        if prev_nl == 0 {
            break;
        }
        let mut prev_start = prev_nl;
        while prev_start > 0 && bytes[prev_start - 1] != b'\n' {
            prev_start -= 1;
        }
        let line = &content[prev_start..prev_nl];
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
            cluster_start = prev_start;
        } else {
            break;
        }
    }

    // --- Walk FORWARDS to find cluster_end ---
    // Find the end of the derive's line first, then advance line-by-line while
    // the next line is another attribute.
    let mut cursor = derive_offset;
    while cursor < content.len() && bytes[cursor] != b'\n' {
        cursor += 1;
    }
    // `cursor` is at the '\n' ending the derive line (or content.len()).
    let mut cluster_end = cursor;
    while cursor < content.len() {
        // Advance past the '\n'.
        let line_start = cursor + 1;
        if line_start >= content.len() {
            break;
        }
        // Find end of this line.
        let mut line_end = line_start;
        while line_end < content.len() && bytes[line_end] != b'\n' {
            line_end += 1;
        }
        let line = &content[line_start..line_end];
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
            cluster_end = line_end;
            cursor = line_end;
        } else {
            break;
        }
    }

    (cluster_start, cluster_end)
}

#[derive(Debug)]
struct ForbiddenCombo {
    file: PathBuf,
    line: usize,
    reason: String,
}

#[test]
fn enum_tagging_audit_no_adjacent_or_flatten_on_ts_derives() {
    // Resolve the rc-common crate src dir relative to CARGO_MANIFEST_DIR.
    let manifest_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    assert!(
        src_dir.is_dir(),
        "expected rc-common src dir at {:?}",
        src_dir
    );

    let files = collect_rs_files(&src_dir);
    assert!(
        !files.is_empty(),
        "expected at least one .rs file under {:?}",
        src_dir
    );

    // Match `#[derive(...TS...)]` OR `#[cfg_attr(feature = "ts-rs", derive(...TS...))]`.
    // We look for the literal identifier `TS` (case sensitive, word-bounded).
    let ts_derive_re = Regex::new(
        r#"#\[(?:cfg_attr\([^)]*derive\([^)]*\bTS\b[^)]*\)\)|derive\([^)]*\bTS\b[^)]*\))\]"#,
    )
    .expect("ts_derive_re compiles");

    // Forbidden-combo detectors (operate on the attribute cluster + block body).
    let serde_tag_content_re =
        Regex::new(r#"#\[serde\([^\]]*\btag\s*=\s*"[^"]+""#).expect("tag re compiles");
    let serde_flatten_re =
        Regex::new(r#"#\[serde\([^\]]*\bflatten\b"#).expect("flatten re compiles");

    let mut files_scanned = 0usize;
    let mut ts_sites_scanned = 0usize;
    let mut forbidden: Vec<ForbiddenCombo> = Vec::new();

    for file in &files {
        files_scanned += 1;
        let content = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for m in ts_derive_re.find_iter(&content) {
            ts_sites_scanned += 1;
            let derive_start = m.start();
            let (cluster_start, cluster_end) = find_attribute_cluster(&content, derive_start);

            // Attribute cluster covers every consecutive `#[...]` line above
            // AND below the derive, stopping at the first non-attribute line
            // (usually the `pub struct` / `pub enum` header itself).
            let cluster = &content[cluster_start..cluster_end];

            // Check attributes surrounding the derive for forbidden combinations.
            if let Some(tm) = serde_tag_content_re.find(cluster) {
                let abs = cluster_start + tm.start();
                let (line, _col) = offset_to_line_col(&content, abs);
                forbidden.push(ForbiddenCombo {
                    file: file.clone(),
                    line,
                    reason: "TS-derive combined with #[serde(tag = ...)] (adjacently/internally tagged enum — D-14 SKIP-list)"
                        .to_string(),
                });
            }

            // Scan the block body for field-level `#[serde(flatten)]`.
            if let Some((body_start, body_end)) = find_block_after(&content, derive_start) {
                let body = &content[body_start..=body_end.min(content.len().saturating_sub(1))];
                if let Some(fm) = serde_flatten_re.find(body) {
                    let abs = body_start + fm.start();
                    let (line, _col) = offset_to_line_col(&content, abs);
                    forbidden.push(ForbiddenCombo {
                        file: file.clone(),
                        line,
                        reason: "TS-derive on a struct/enum whose body contains #[serde(flatten)] (D-14 SKIP-list)"
                            .to_string(),
                    });
                }
            }
        }
    }

    if !forbidden.is_empty() {
        let mut msg = String::from(
            "enum_tagging_audit: FORBIDDEN ts-rs + serde-tag/flatten combinations detected.\n",
        );
        for combo in &forbidden {
            msg.push_str(&format!(
                "  - {}:{} — {}\n",
                combo.file.display(),
                combo.line,
                combo.reason
            ));
        }
        msg.push_str(
            "\nSee .planning/phases/445-typed-api-contract-rust-ts-codegen/445-CONTEXT.md D-14.\n",
        );
        panic!("{msg}");
    }

    println!(
        "enum_tagging_audit: scanned {files_scanned} files, {ts_sites_scanned} TS-derived sites, 0 forbidden combos"
    );
}
