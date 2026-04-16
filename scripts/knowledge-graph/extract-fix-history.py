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
from collections import defaultdict
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
GRAPH_PATH = REPO_ROOT / ".planning" / "knowledge-graph" / "graph.json"
OUTPUT_PATH = REPO_ROOT / ".planning" / "knowledge-graph" / "fix-history.json"
LOGBOOK_PATH = REPO_ROOT / "LOGBOOK.md"
ERROR_CATALOG_PATH = REPO_ROOT / "docs" / "ERROR-CATALOG.md"

# Symptom keyword patterns — extracted from common RacingPoint incident vocabulary
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
    (r"pricing|price|rate", "pricing"),

    # Game launch
    (r"launch", "game-launch"),
    (r"ac(?:s)?\.exe|assetto", "assetto-corsa"),
    (r"race\.ini|config.?mismatch", "game-config"),
    (r"telemetry|udp", "telemetry"),
    (r"ffb|force.?feedback|conspit", "ffb-wheelbase"),

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

    # Infrastructure
    (r"tailscale|ssh", "remote-access"),
    (r"pod.?\d", "pod-specific"),
    (r"fleet", "fleet-wide"),
    (r"cloud|bono.?vps", "cloud"),
    (r"comms.?link", "comms-link"),
]


def get_fix_commits():
    """Get all fix-related commits with their changed files."""
    result = subprocess.run(
        ["git", "log", "--all", "--format=%H|||%s|||%ai", "--name-only",
         "--diff-filter=ACDMR"],
        capture_output=True, text=True, cwd=REPO_ROOT
    )

    commits = []
    current = None

    for line in result.stdout.split("\n"):
        if "|||" in line:
            if current and current["files"]:
                commits.append(current)
            parts = line.split("|||")
            if len(parts) >= 3:
                msg = parts[1].strip().lower()
                # Filter to fix-related commits
                is_fix = any(kw in msg for kw in [
                    "fix", "bug", "crash", "broken", "revert", "hotfix",
                    "repair", "patch", "resolve", "workaround", "recovery",
                    "guard", "safety", "sentinel", "maintenance"
                ])
                current = {
                    "hash": parts[0][:8],
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
        commits.append(current)

    return [c for c in commits if c["is_fix"]]


def extract_symptoms(message):
    """Extract symptom keywords from a commit message."""
    symptoms = set()
    msg_lower = message.lower()
    for pattern, label in SYMPTOM_PATTERNS:
        if re.search(pattern, msg_lower):
            symptoms.add(label)
    return list(symptoms)


def load_graph_nodes():
    """Load graph nodes and build a file→nodes lookup."""
    if not GRAPH_PATH.exists():
        print(f"ERROR: graph.json not found at {GRAPH_PATH}")
        return {}, {}

    with open(GRAPH_PATH) as f:
        graph = json.load(f)

    file_to_nodes = defaultdict(list)
    node_index = {}

    for node in graph.get("nodes", []):
        node_id = node.get("label", node.get("id", ""))
        source = node.get("source_file", node.get("source", ""))
        community = node.get("community", -1)

        node_info = {
            "id": node_id,
            "source": source,
            "community": community,
            "type": node.get("file_type", "unknown"),
            "loc": node.get("source_location", ""),
        }
        node_index[node_id] = node_info

        if source:
            # Normalize path — strip leading ./
            norm_source = source.lstrip("./")
            file_to_nodes[norm_source].append(node_id)
            # Also index by filename only for broader matching
            basename = os.path.basename(norm_source)
            file_to_nodes[f"_basename_{basename}"].append(node_id)

    return file_to_nodes, node_index


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
            # Format: | date | author | hash | description |
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


def build_fix_overlay(commits, file_to_nodes, node_index, logbook_entries, error_entries):
    """Build the fix-history overlay: graph nodes annotated with fix history."""

    # Node → fix history
    node_fixes = defaultdict(lambda: {
        "fix_count": 0,
        "symptoms": set(),
        "fixes": [],
        "last_fixed": None,
        "communities_affected": set(),
    })

    # Symptom → nodes index (for reverse lookup)
    symptom_index = defaultdict(set)

    for commit in commits:
        symptoms = extract_symptoms(commit["message"])

        for filepath in commit["files"]:
            norm_path = filepath.lstrip("./")
            matched_nodes = file_to_nodes.get(norm_path, [])

            # Also try basename match
            if not matched_nodes:
                basename = os.path.basename(norm_path)
                matched_nodes = file_to_nodes.get(f"_basename_{basename}", [])

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

                # Build reverse index
                for symptom in symptoms:
                    symptom_index[symptom].add(node_id)

    # Enrich from LOGBOOK
    for entry in logbook_entries:
        for symptom in entry["symptoms"]:
            # Find nodes that match this symptom
            for node_id in symptom_index.get(symptom, set()):
                node_fixes[node_id]["symptoms"].update(entry["symptoms"])

    # Enrich from ERROR-CATALOG
    for error in error_entries:
        for symptom in error["symptoms"]:
            for node_id in symptom_index.get(symptom, set()):
                node_fixes[node_id]["symptoms"].update(error["symptoms"])

    # Convert sets to lists for JSON serialization
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
    """Find the most frequently fixed nodes — these are your structural weak points."""
    hotspots = sorted(
        node_fixes.items(),
        key=lambda x: x[1]["fix_count"],
        reverse=True
    )[:30]
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
    file_to_nodes, node_index = load_graph_nodes()
    print(f"  Loaded {len(node_index)} nodes, {len(file_to_nodes)} file mappings")

    print("[2/5] Extracting fix commits from git...")
    commits = get_fix_commits()
    print(f"  Found {len(commits)} fix-related commits")

    print("[3/5] Parsing LOGBOOK and ERROR-CATALOG...")
    logbook_entries = parse_logbook()
    error_entries = parse_error_catalog()
    print(f"  LOGBOOK: {len(logbook_entries)} fix entries")
    print(f"  ERROR-CATALOG: {len(error_entries)} known errors")

    print("[4/5] Building fix overlay...")
    node_fixes, symptom_index = build_fix_overlay(
        commits, file_to_nodes, node_index, logbook_entries, error_entries
    )
    hotspots = compute_hotspots(node_fixes)
    print(f"  Annotated {len(node_fixes)} nodes with fix history")
    print(f"  Created {len(symptom_index)} symptom lookups")

    print("[5/5] Writing fix-history.json...")
    output = {
        "metadata": {
            "generated": subprocess.run(
                ["date", "-u", "+%Y-%m-%dT%H:%M:%SZ"],
                capture_output=True, text=True
            ).stdout.strip(),
            "fix_commits_analyzed": len(commits),
            "logbook_entries": len(logbook_entries),
            "error_catalog_entries": len(error_entries),
            "nodes_annotated": len(node_fixes),
            "symptom_count": len(symptom_index),
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
