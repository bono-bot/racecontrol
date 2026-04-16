#!/usr/bin/env python3
"""
Extract fix history from git log and overlay it onto the Graphify knowledge graph.

Walks all fix-related commits, maps changed files to graph nodes, extracts
symptom keywords from commit messages, and produces a fix-history overlay
that enables symptom → node → fix lookups.

Output: .planning/knowledge-graph/fix-history.json
"""

import json
import subprocess
import re
import os
import shutil
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
KG_DIR = REPO_ROOT / ".planning" / "knowledge-graph"
GRAPH_PATH = KG_DIR / "graph.json"
OUTPUT_PATH = KG_DIR / "fix-history.json"
LOGBOOK_PATH = REPO_ROOT / "LOGBOOK.md"
ERROR_CATALOG_PATH = REPO_ROOT / "docs" / "ERROR-CATALOG.md"
# Also check graphify-out/ for graph.json (G3: auto-copy)
GRAPHIFY_OUT_PATH = REPO_ROOT / "graphify-out" / "graph.json"

# Symptom keyword patterns — extracted from common RacingPoint incident vocabulary
# Keep in sync with query-fixes.py SYMPTOM_PATTERNS (G4)
SYMPTOM_PATTERNS = [
    # Process/restart issues
    (r"crash\s*loop", "crash-loop"),
    (r"restart", "restart"),
    (r"maintenance.?mode", "maintenance-mode"),
    (r"session.?0", "session-0"),
    (r"watchdog", "watchdog"),
    (r"orphan", "orphan-process"),
    (r"zombie", "zombie-process"),
    (r"taskkill", "process-kill"),
    (r"schtask", "scheduled-task"),

    # Network/connectivity
    (r"websocket|ws.?connect|ws.?disconnect", "websocket"),
    (r"401|unauthorized|auth", "auth-failure"),
    (r"403|forbidden", "forbidden"),
    (r"timeout", "timeout"),
    (r"connection.?refuse", "connection-refused"),
    (r"port.?conflict|10048|address.?in.?use", "port-conflict"),

    # Display/UI
    (r"blank(?:ing)?(?:\s+screen)?", "blanking-screen"),
    (r"flicker", "flicker"),
    (r"1024x768|resolution|nvidia.?surround", "display-resolution"),
    (r"kiosk", "kiosk"),
    (r"edge.?browser", "edge-browser"),
    (r"overlay", "overlay"),

    # Billing/sessions
    (r"billing", "billing"),
    (r"wallet|credit|debit", "wallet"),
    (r"refund", "refund"),
    (r"session.?end|end.?session", "session-end"),
    (r"pricing|price|rate|tier", "pricing"),

    # Game launch
    (r"launch", "game-launch"),
    (r"ac(?:s)?\.exe|assetto", "assetto-corsa"),
    (r"race\.ini|config.?mismatch", "game-config"),
    (r"telemetry|udp", "telemetry"),
    (r"ffb|force.?feedback|conspit", "ffb-wheelbase"),
    (r"zero.?lap|no.?lap|0.?lap", "zero-laps"),
    (r"python\.ini", "python-ini"),
    (r"f1.?25|ea.?app", "f1-25"),
    (r"iracing", "iracing"),
    (r"le.?mans|lmu", "lmu"),
    (r"forza", "forza"),

    # Deploy
    (r"deploy", "deploy"),
    (r"binary|build.?id", "binary"),
    (r"swap|rollback|prev", "binary-swap"),
    (r"bat\s+file|\.bat", "bat-file"),
    (r"stale|outdated|old.?build", "stale-build"),

    # Config
    (r"toml|config", "config"),
    (r"allowlist|whitelist", "allowlist"),
    (r"sentry.?key|service.?key", "service-key"),
    (r"feature.?flag", "feature-flag"),
    (r"foreign.?key|fk.?constraint|\bfk\b", "foreign-key"),

    # Infrastructure
    (r"tailscale|ssh", "remote-access"),
    (r"pod.?\d", "pod-specific"),
    (r"fleet", "fleet-wide"),
    (r"cloud|bono.?vps", "cloud"),
    (r"comms.?link", "comms-link"),
    (r"memory.?leak|powershell.?leak", "memory-leak"),
]

# Generic node names that should never be hotspots — they appear in every file
GENERIC_NODE_NAMES = frozenset({
    ".default()", ".new()", ".name()", ".run()", "ok()",
    "GET()", "spawn()", "main()", "entry()", "load()",
    ".push()", ".get()", ".set()", ".collect()", ".clone()",
    ".as_str()", ".open()", ".into()", ".map()", ".unwrap()",
    ".drop()", ".fmt()", ".from()", ".try_from()", ".deref()",
    "Error", "Display", "Debug", "Clone", "Default",
})

GENERIC_FILE_NAMES = frozenset({
    "mod.rs", "page.tsx", "main.rs", "lib.rs",
    "index.ts", "index.js", "layout.tsx", "route.ts",
})


def _normalize_path(filepath):
    """Normalize file paths for cross-platform matching.

    Git uses forward slashes. Graphify on Windows uses backslashes.
    Both may have leading ./ or no prefix. Normalize to forward slashes, no prefix.
    """
    return filepath.replace("\\", "/").lstrip("./")


def get_fix_commits():
    """Get all fix-related commits with their changed files AND diff line ranges."""
    result = subprocess.run(
        ["git", "log", "--all", "--format=%H|||%s|||%ai", "--name-only",
         "--diff-filter=ACDMR"],
        capture_output=True, text=True, cwd=REPO_ROOT
    )

    raw_commits = []
    current = None

    for line in result.stdout.split("\n"):
        if "|||" in line:
            if current and current["files"]:
                raw_commits.append(current)
            parts = line.split("|||")
            if len(parts) >= 3:
                msg = parts[1].strip().lower()
                is_fix = any(kw in msg for kw in [
                    "fix", "bug", "crash", "broken", "revert", "hotfix",
                    "repair", "patch", "resolve", "workaround", "recovery",
                    "guard", "safety", "sentinel", "maintenance"
                ])
                current = {
                    "hash": parts[0][:8],
                    "full_hash": parts[0],
                    "message": parts[1].strip(),
                    "date": parts[2].strip()[:10],
                    "files": [],
                    "is_fix": is_fix
                }
            else:
                current = None
        elif line.strip() and current:
            current["files"].append(line.strip())

    if current and current.get("files"):
        raw_commits.append(current)

    fix_commits = [c for c in raw_commits if c["is_fix"]]

    # Second pass: get diff hunks for function-level mapping
    print(f"  Extracting diff hunks for {len(fix_commits)} fix commits...")
    for i, commit in enumerate(fix_commits):
        if i % 200 == 0 and i > 0:
            print(f"    ... {i}/{len(fix_commits)} commits processed")
        commit["hunks"] = _get_diff_hunks(commit["full_hash"])

    return fix_commits


def _get_diff_hunks(commit_hash):
    """Extract changed line ranges per file from a commit's diff."""
    try:
        result = subprocess.run(
            ["git", "diff", f"{commit_hash}~1..{commit_hash}", "--unified=0", "--no-color",
             "--diff-filter=ACDMR", "--no-ext-diff"],
            capture_output=True, cwd=REPO_ROOT,
            timeout=10
        )
    except (subprocess.TimeoutExpired, Exception):
        return {}
    if result.returncode != 0:
        return {}

    try:
        stdout = result.stdout.decode("utf-8", errors="replace")
    except Exception:
        return {}

    hunks = defaultdict(list)
    current_file = None

    for line in stdout.split("\n"):
        if line.startswith("+++ b/"):
            current_file = _normalize_path(line[6:])
        elif line.startswith("@@") and current_file:
            match = re.search(r'\+(\d+)(?:,(\d+))?', line)
            if match:
                start = int(match.group(1))
                count = int(match.group(2) or 1)
                end = start + max(count - 1, 0)
                hunks[current_file].append((start, end))

    return dict(hunks)


def extract_symptoms(message):
    """Extract symptom keywords from a commit message."""
    symptoms = set()
    msg_lower = message.lower()
    for pattern, label in SYMPTOM_PATTERNS:
        if re.search(pattern, msg_lower):
            symptoms.add(label)
    return list(symptoms)


def _auto_copy_graph():
    """G3 fix: Auto-copy graph.json from graphify-out/ if it's newer."""
    if not GRAPHIFY_OUT_PATH.exists():
        return
    if not GRAPH_PATH.exists():
        print(f"  Auto-copying graph.json from graphify-out/")
        shutil.copy2(GRAPHIFY_OUT_PATH, GRAPH_PATH)
        return
    # Copy if graphify-out version is newer
    out_mtime = GRAPHIFY_OUT_PATH.stat().st_mtime
    kg_mtime = GRAPH_PATH.stat().st_mtime
    if out_mtime > kg_mtime:
        print(f"  Auto-copying newer graph.json from graphify-out/")
        shutil.copy2(GRAPHIFY_OUT_PATH, GRAPH_PATH)


def load_graph_nodes():
    """Load graph nodes and build file->nodes + file->line->nodes lookups.

    G1 fix: Normalize all paths to forward slashes. No basename fallback.
    """
    _auto_copy_graph()

    if not GRAPH_PATH.exists():
        print(f"ERROR: graph.json not found at {GRAPH_PATH}")
        print(f"  Run: graphify update . (in repo root)")
        return {}, {}, {}

    with open(GRAPH_PATH) as f:
        graph = json.load(f)

    file_to_nodes = defaultdict(list)
    node_index = {}
    file_line_index = defaultdict(list)

    for node in graph.get("nodes", []):
        node_id = node.get("label", node.get("id", ""))
        source = node.get("source_file", node.get("source", ""))
        community = node.get("community", -1)
        loc = node.get("source_location", "")

        line_num = 0
        if loc and loc.startswith("L"):
            try:
                line_num = int(loc[1:])
            except ValueError:
                pass

        node_info = {
            "id": node_id,
            "source": source,
            "community": community,
            "type": node.get("file_type", "unknown"),
            "loc": loc,
            "line": line_num,
        }
        node_index[node_id] = node_info

        if source:
            # G1: Normalize to forward slashes, strip leading ./
            norm_source = _normalize_path(source)
            file_to_nodes[norm_source].append(node_id)
            # NO basename fallback — this caused massive over-counting

            if line_num > 0:
                file_line_index[norm_source].append((line_num, node_id))

    for filepath in file_line_index:
        file_line_index[filepath].sort(key=lambda x: x[0])

    return file_to_nodes, node_index, file_line_index


def parse_logbook():
    """Extract fix entries from LOGBOOK.md."""
    entries = []
    if not LOGBOOK_PATH.exists():
        return entries

    with open(LOGBOOK_PATH) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or line.startswith("|--"):
                continue
            parts = [p.strip() for p in line.split("|") if p.strip()]
            if len(parts) >= 4:
                desc = parts[3].lower()
                if any(kw in desc for kw in ["fix", "bug", "crash", "broken", "repair", "patch"]):
                    entries.append({
                        "date": parts[0],
                        "author": parts[1],
                        "hash": parts[2],
                        "description": parts[3],
                        "symptoms": extract_symptoms(parts[3])
                    })
    return entries


def parse_error_catalog():
    """Extract known errors from ERROR-CATALOG.md and their symptoms."""
    errors = []
    if not ERROR_CATALOG_PATH.exists():
        return errors

    current = None
    with open(ERROR_CATALOG_PATH) as f:
        for line in f:
            if line.startswith("### "):
                if current:
                    errors.append(current)
                current = {
                    "title": line[4:].strip(),
                    "symptoms": extract_symptoms(line),
                    "body": ""
                }
            elif current:
                current["body"] += line
                current["symptoms"] = list(set(
                    current["symptoms"] + extract_symptoms(line)
                ))
    if current:
        errors.append(current)
    return errors


def _find_nodes_in_range(file_line_index, filepath, hunk_start, hunk_end):
    """Find graph nodes whose start line falls within a diff hunk range."""
    MARGIN = 50
    entries = file_line_index.get(filepath, [])
    if not entries:
        return []

    matched = []
    for node_line, node_id in entries:
        if (hunk_start - MARGIN) <= node_line <= (hunk_end + MARGIN):
            matched.append(node_id)
    return matched


def build_fix_overlay(commits, file_to_nodes, node_index, file_line_index, logbook_entries, error_entries):
    """Build the fix-history overlay: graph nodes annotated with fix history.

    G1 fix: Uses normalized paths for matching. No basename fallback.
    Commits that don't match any graph file are tracked separately for diagnostics.
    """
    node_fixes = defaultdict(lambda: {
        "fix_count": 0,
        "symptoms": set(),
        "fixes": [],
        "last_fixed": None,
        "communities_affected": set(),
    })

    symptom_index = defaultdict(set)

    hunk_hits = 0
    file_hits = 0
    unmatched_files = 0

    for commit in commits:
        symptoms = extract_symptoms(commit["message"])
        hunks = commit.get("hunks", {})

        for filepath in commit["files"]:
            norm_path = _normalize_path(filepath)

            matched_nodes = []
            file_hunks = hunks.get(norm_path, [])

            # Try function-level matching via diff hunks first
            if file_hunks and norm_path in file_line_index:
                for hunk_start, hunk_end in file_hunks:
                    matched_nodes.extend(
                        _find_nodes_in_range(file_line_index, norm_path, hunk_start, hunk_end)
                    )
                matched_nodes = list(set(matched_nodes))
                if matched_nodes:
                    hunk_hits += 1

            # Fall back to file-level matching (same normalized path only — NO basename)
            if not matched_nodes:
                matched_nodes = file_to_nodes.get(norm_path, [])
                if matched_nodes:
                    file_hits += 1
                else:
                    unmatched_files += 1

            for node_id in matched_nodes:
                entry = node_fixes[node_id]
                entry["fix_count"] += 1
                entry["symptoms"].update(symptoms)
                entry["fixes"].append({
                    "hash": commit["hash"],
                    "date": commit["date"],
                    "message": commit["message"][:200],
                })
                entry["last_fixed"] = commit["date"]

                if node_id in node_index:
                    entry["communities_affected"].add(node_index[node_id]["community"])

                for symptom in symptoms:
                    symptom_index[symptom].add(node_id)

    print(f"  Function-level hunk matches: {hunk_hits} | File-level matches: {file_hits} | Unmatched files: {unmatched_files}")

    # Enrich from LOGBOOK — only add symptoms to nodes already in symptom_index
    for entry in logbook_entries:
        for symptom in entry["symptoms"]:
            for node_id in list(symptom_index.get(symptom, set())):
                node_fixes[node_id]["symptoms"].update(entry["symptoms"])

    # Enrich from ERROR-CATALOG
    for error in error_entries:
        for symptom in error["symptoms"]:
            for node_id in list(symptom_index.get(symptom, set())):
                node_fixes[node_id]["symptoms"].update(error["symptoms"])

    # Convert sets to lists for JSON
    serializable_fixes = {}
    for node_id, data in node_fixes.items():
        serializable_fixes[node_id] = {
            "fix_count": data["fix_count"],
            "symptoms": sorted(data["symptoms"]),
            "fixes": sorted(data["fixes"], key=lambda x: x["date"], reverse=True)[:10],
            "last_fixed": data["last_fixed"],
            "communities_affected": sorted(data["communities_affected"]),
        }

    serializable_symptom_index = {
        symptom: sorted(nodes)
        for symptom, nodes in symptom_index.items()
    }

    return serializable_fixes, serializable_symptom_index


def compute_hotspots(node_fixes):
    """Find the most frequently fixed nodes — excluding generic names (G6 fix)."""
    candidates = [
        (nid, data) for nid, data in node_fixes.items()
        if nid not in GENERIC_NODE_NAMES
        and nid not in GENERIC_FILE_NAMES
        and not nid.startswith(".")  # skip .method() style generic names
    ]
    hotspots = sorted(candidates, key=lambda x: x[1]["fix_count"], reverse=True)[:30]
    return [
        {
            "node": node_id,
            "fix_count": data["fix_count"],
            "symptoms": data["symptoms"][:5],
            "last_fixed": data["last_fixed"],
        }
        for node_id, data in hotspots
    ]


def main():
    print("=== RacingPoint Fix-History Knowledge Graph Overlay ===\n")

    print("[1/5] Loading graph nodes...")
    file_to_nodes, node_index, file_line_index = load_graph_nodes()
    print(f"  Loaded {len(node_index)} nodes, {len(file_to_nodes)} file mappings, {len(file_line_index)} files with line-level index")

    print("[2/5] Extracting fix commits from git (with diff hunks)...")
    commits = get_fix_commits()
    print(f"  Found {len(commits)} fix-related commits")

    print("[3/5] Parsing LOGBOOK and ERROR-CATALOG...")
    logbook_entries = parse_logbook()
    error_entries = parse_error_catalog()
    print(f"  LOGBOOK: {len(logbook_entries)} fix entries")
    print(f"  ERROR-CATALOG: {len(error_entries)} known errors")

    print("[4/5] Building fix overlay (function-level matching)...")
    node_fixes, symptom_index = build_fix_overlay(
        commits, file_to_nodes, node_index, file_line_index, logbook_entries, error_entries
    )
    hotspots = compute_hotspots(node_fixes)
    print(f"  Annotated {len(node_fixes)} nodes with fix history")
    print(f"  Created {len(symptom_index)} symptom lookups")

    # G8 fix: Use Python datetime instead of shell `date` command
    print("[5/5] Writing fix-history.json...")
    output = {
        "metadata": {
            "generated": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "fix_commits_analyzed": len(commits),
            "logbook_entries": len(logbook_entries),
            "error_catalog_entries": len(error_entries),
            "nodes_annotated": len(node_fixes),
            "symptom_count": len(symptom_index),
            "graph_json_mtime": datetime.fromtimestamp(
                GRAPH_PATH.stat().st_mtime, tz=timezone.utc
            ).strftime("%Y-%m-%dT%H:%M:%SZ") if GRAPH_PATH.exists() else None,
        },
        "hotspots": hotspots,
        "symptom_index": symptom_index,
        "node_fixes": node_fixes,
    }

    with open(OUTPUT_PATH, "w") as f:
        json.dump(output, f, indent=2)

    size_mb = os.path.getsize(OUTPUT_PATH) / (1024 * 1024)
    print(f"\n  Written to {OUTPUT_PATH} ({size_mb:.1f} MB)")

    print("\n=== Top 10 Hotspots (most-fixed nodes) ===")
    for i, h in enumerate(hotspots[:10], 1):
        symptoms_str = ", ".join(h["symptoms"][:3])
        print(f"  {i}. {h['node']} — {h['fix_count']} fixes [{symptoms_str}]")

    print("\n=== Symptom Coverage ===")
    for symptom, nodes in sorted(symptom_index.items(), key=lambda x: -len(x[1]))[:15]:
        print(f"  {symptom}: {len(nodes)} nodes")


if __name__ == "__main__":
    main()
