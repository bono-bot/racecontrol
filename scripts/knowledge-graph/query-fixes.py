#!/usr/bin/env python3
"""
Query the fix-history knowledge graph overlay.

Usage:
  python query-fixes.py "pod keeps restarting"
  python query-fixes.py "billing refund wrong amount"
  python query-fixes.py "screens went to 1024x768"
  python query-fixes.py --hotspots          # show most-fixed nodes
  python query-fixes.py --symptom restart   # lookup by exact symptom key

Searches symptom index, finds related graph nodes, shows past fixes
and blast radius (which communities were affected).
"""

import json
import sys
import re
import os
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
FIX_HISTORY_PATH = REPO_ROOT / ".planning" / "knowledge-graph" / "fix-history.json"

# Same patterns as extract script — for query-time matching
SYMPTOM_PATTERNS = [
    (r"crash\s*loop", "crash-loop"),
    (r"restart", "restart"),
    (r"maintenance.?mode", "maintenance-mode"),
    (r"session.?0", "session-0"),
    (r"watchdog", "watchdog"),
    (r"orphan", "orphan-process"),
    (r"zombie", "zombie-process"),
    (r"websocket|ws.?connect|ws.?disconnect", "websocket"),
    (r"401|unauthorized|auth", "auth-failure"),
    (r"timeout", "timeout"),
    (r"port.?conflict|10048", "port-conflict"),
    (r"blank(?:ing)?(?:\s+screen)?", "blanking-screen"),
    (r"flicker", "flicker"),
    (r"1024x768|resolution|nvidia", "display-resolution"),
    (r"kiosk", "kiosk"),
    (r"billing", "billing"),
    (r"wallet|credit|debit", "wallet"),
    (r"refund", "refund"),
    (r"pricing|price|rate", "pricing"),
    (r"launch", "game-launch"),
    (r"assetto|ac\.exe", "assetto-corsa"),
    (r"race\.ini|config.?mismatch", "game-config"),
    (r"telemetry|udp", "telemetry"),
    (r"ffb|force.?feedback|conspit", "ffb-wheelbase"),
    (r"deploy", "deploy"),
    (r"binary|build.?id", "binary"),
    (r"bat\s+file|\.bat", "bat-file"),
    (r"stale|outdated", "stale-build"),
    (r"toml|config", "config"),
    (r"allowlist|whitelist", "allowlist"),
    (r"sentry.?key", "service-key"),
    (r"tailscale|ssh", "remote-access"),
    (r"pod.?\d", "pod-specific"),
    (r"fleet", "fleet-wide"),
    (r"cloud", "cloud"),
    (r"comms.?link", "comms-link"),
]


def load_fix_history():
    if not FIX_HISTORY_PATH.exists():
        print(f"ERROR: fix-history.json not found at {FIX_HISTORY_PATH}")
        print("Run: python scripts/knowledge-graph/extract-fix-history.py")
        sys.exit(1)
    with open(FIX_HISTORY_PATH) as f:
        return json.load(f)


def query_to_symptoms(query):
    """Convert a natural language query to symptom keys."""
    symptoms = set()
    query_lower = query.lower()
    for pattern, label in SYMPTOM_PATTERNS:
        if re.search(pattern, query_lower):
            symptoms.add(label)

    # Also try direct word matching against symptom index
    words = set(re.findall(r'\w+', query_lower))
    return symptoms, words


def search(query, data):
    """Search fix history by symptom query."""
    symptoms, words = query_to_symptoms(query)
    symptom_index = data["symptom_index"]
    node_fixes = data["node_fixes"]

    # Collect matching nodes from symptom index
    matching_nodes = set()
    matched_symptoms = set()

    for symptom in symptoms:
        if symptom in symptom_index:
            matching_nodes.update(symptom_index[symptom])
            matched_symptoms.add(symptom)

    # Also try fuzzy word match against symptom keys
    for word in words:
        for symptom_key in symptom_index:
            if word in symptom_key or symptom_key in word:
                matching_nodes.update(symptom_index[symptom_key])
                matched_symptoms.add(symptom_key)

    if not matching_nodes:
        print(f"\nNo matches for: \"{query}\"")
        print(f"Symptom keys tried: {sorted(symptoms)}")
        print(f"\nAvailable symptom keys:")
        for key in sorted(symptom_index.keys()):
            print(f"  - {key} ({len(symptom_index[key])} nodes)")
        return

    # Rank nodes by SPECIFICITY-WEIGHTED fix count
    # Nodes that appear in many symptom categories are generic (mod.rs, page.tsx)
    # Nodes that appear in few categories are specific to this problem
    total_symptoms = len(symptom_index)
    ranked = []
    for node_id in matching_nodes:
        if node_id in node_fixes:
            fix_data = node_fixes[node_id]
            # Count how many symptom categories this node appears in
            node_symptom_breadth = len(fix_data.get("symptoms", []))
            # Specificity: nodes in fewer categories score higher
            # A node in 2/41 symptoms is highly specific; one in 40/41 is generic
            specificity = max(0.1, 1.0 - (node_symptom_breadth / max(total_symptoms, 1)))
            # Skip obviously generic nodes (stdlib-level names)
            if node_id in (".default()", ".new()", ".name()", ".run()", "ok()",
                           "GET()", "spawn()", "main()", "entry()", "load()",
                           ".push()", ".get()", ".set()", ".collect()", ".clone()",
                           ".as_str()", ".open()", ".into()", ".map()", ".unwrap()"):
                specificity *= 0.01
            # Generic filenames that appear everywhere
            if node_id in ("mod.rs", "page.tsx", "main.rs", "routes.rs",
                           "lib.rs", "index.ts", "layout.tsx"):
                specificity *= 0.05
            score = fix_data["fix_count"] * specificity
            ranked.append((node_id, fix_data, score))

    ranked.sort(key=lambda x: x[2], reverse=True)

    # Display results
    print(f"\n{'='*60}")
    print(f"QUERY: \"{query}\"")
    print(f"MATCHED SYMPTOMS: {', '.join(sorted(matched_symptoms))}")
    print(f"NODES FOUND: {len(ranked)}")
    print(f"{'='*60}")

    # Top 15 most-relevant nodes
    print(f"\n--- Top Results (by relevance score) ---\n")
    for i, (node_id, fix_data, score) in enumerate(ranked[:15], 1):
        print(f"  {i}. {node_id}  (score: {score:.1f})")
        print(f"     Fixes: {fix_data['fix_count']} | "
              f"Last: {fix_data['last_fixed']} | "
              f"Communities: {fix_data['communities_affected']}")
        print(f"     Symptoms: {', '.join(fix_data['symptoms'][:5])}")
        if fix_data["fixes"]:
            latest = fix_data["fixes"][0]
            print(f"     Latest fix: [{latest['hash']}] {latest['message'][:80]}")
        print()

    # Blast radius — which communities are affected
    affected_communities = set()
    for _, fix_data, _ in ranked:
        affected_communities.update(fix_data.get("communities_affected", []))

    if affected_communities:
        print(f"--- Blast Radius ---")
        print(f"Communities affected: {sorted(affected_communities)}")
        print(f"When fixing this area, also check nodes in these clusters.\n")

    # Recent fixes across all matched nodes
    all_fixes = []
    for _, fix_data, _ in ranked:
        all_fixes.extend(fix_data.get("fixes", []))
    all_fixes.sort(key=lambda x: x["date"], reverse=True)

    seen_hashes = set()
    unique_fixes = []
    for fix in all_fixes:
        if fix["hash"] not in seen_hashes:
            seen_hashes.add(fix["hash"])
            unique_fixes.append(fix)

    # Direct commit message search — most actionable result
    # Search ALL fix commits for query words (not just via symptom index)
    query_words = set(w for w in re.findall(r'\w{3,}', query.lower())
                      if w not in ("the", "and", "for", "was", "not", "has", "are",
                                   "with", "this", "that", "from", "but", "have"))
    direct_matches = []
    for _, fix_data, _ in ranked:
        for fix in fix_data.get("fixes", []):
            msg_lower = fix["message"].lower()
            match_count = sum(1 for w in query_words if w in msg_lower)
            if match_count >= 1:
                direct_matches.append((fix, match_count))

    # Deduplicate by hash
    seen_direct = set()
    unique_direct = []
    for fix, score in sorted(direct_matches, key=lambda x: (-x[1], x[0]["date"]), reverse=False):
        if fix["hash"] not in seen_direct:
            seen_direct.add(fix["hash"])
            unique_direct.append((fix, score))
    unique_direct.sort(key=lambda x: (-x[1], x[0]["date"]))

    if unique_direct:
        print(f"--- Most Relevant Commits (keyword match) ---\n")
        for fix, score in unique_direct[:10]:
            print(f"  [{fix['hash']}] {fix['date']} — {fix['message'][:90]}")
        print()

    if unique_fixes:
        print(f"--- Recent Fix Commits (newest first) ---\n")
        for fix in unique_fixes[:10]:
            print(f"  [{fix['hash']}] {fix['date']} — {fix['message'][:90]}")
        print()


def show_hotspots(data):
    """Show the most frequently fixed nodes."""
    print(f"\n{'='*60}")
    print("HOTSPOTS — Most Frequently Fixed Code")
    print(f"{'='*60}\n")

    for i, h in enumerate(data["hotspots"], 1):
        symptoms_str = ", ".join(h["symptoms"][:5])
        print(f"  {i:2}. {h['node']}")
        print(f"      {h['fix_count']} fixes | Last: {h['last_fixed']} | [{symptoms_str}]")
        print()


def show_symptom_key(symptom, data):
    """Show all nodes for a specific symptom key."""
    symptom_index = data["symptom_index"]
    node_fixes = data["node_fixes"]

    if symptom not in symptom_index:
        print(f"Unknown symptom key: {symptom}")
        print(f"Available: {', '.join(sorted(symptom_index.keys()))}")
        return

    nodes = symptom_index[symptom]
    print(f"\n{'='*60}")
    print(f"SYMPTOM: {symptom} — {len(nodes)} nodes")
    print(f"{'='*60}\n")

    ranked = []
    for node_id in nodes:
        if node_id in node_fixes:
            ranked.append((node_id, node_fixes[node_id]["fix_count"]))
    ranked.sort(key=lambda x: x[1], reverse=True)

    for node_id, count in ranked[:20]:
        print(f"  {node_id} — {count} fixes")


def main():
    if len(sys.argv) < 2:
        print("Usage:")
        print('  python query-fixes.py "pod keeps restarting"')
        print('  python query-fixes.py "billing refund wrong"')
        print("  python query-fixes.py --hotspots")
        print("  python query-fixes.py --symptom restart")
        print("  python query-fixes.py --symptoms  # list all symptom keys")
        sys.exit(0)

    data = load_fix_history()

    if sys.argv[1] == "--hotspots":
        show_hotspots(data)
    elif sys.argv[1] == "--symptom" and len(sys.argv) >= 3:
        show_symptom_key(sys.argv[2], data)
    elif sys.argv[1] == "--symptoms":
        print("\nAvailable symptom keys:")
        for key in sorted(data["symptom_index"].keys()):
            print(f"  {key}: {len(data['symptom_index'][key])} nodes")
    else:
        query = " ".join(sys.argv[1:])
        search(query, data)


if __name__ == "__main__":
    main()
