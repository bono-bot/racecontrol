#!/usr/bin/env bash
# deploy-probe.sh — graphify blast-radius probe for deploy-gate enumeration.
#
# Waves 1+2+3+4 — plan_graphify_blast_radius_20260423.md.
# Next waves: E2E cross-ref (W5), graph-diff (W6), symptom probe (W7).
#
# Usage: bash scripts/graphify-helpers/deploy-probe.sh <branch> [flags]
# Flags:
#   --require-fresh            exit 1 if graph older than HEAD by > MAX_AGE
#   --max-age-min N            stale threshold in minutes (default 60)
#   --max-hops N               BFS depth (default 2 — W4 default)
#   --god-node-threshold N     in-degree >= N marks a node as GOD (default 10)
#   --graph-file PATH          override default graph lookup
#   --no-color                 disable ANSI colors
#   -h | --help                print usage
# Exit codes:
#   0 success    1 stale/missing graph (with --require-fresh)    2 query/arg failure

set -euo pipefail

BRANCH=""
REQUIRE_FRESH=0
MAX_HOPS=2
MAX_AGE_MIN=60
GOD_NODE_THRESHOLD=10
GRAPH_FILE=""
USE_COLOR=1

print_usage() { sed -n '3,17p' "$0" | sed 's/^# \{0,1\}//'; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --require-fresh) REQUIRE_FRESH=1; shift ;;
    --max-age-min) MAX_AGE_MIN="$2"; shift 2 ;;
    --max-hops) MAX_HOPS="$2"; shift 2 ;;
    --god-node-threshold) GOD_NODE_THRESHOLD="$2"; shift 2 ;;
    --graph-file) GRAPH_FILE="$2"; shift 2 ;;
    --no-color) USE_COLOR=0; shift ;;
    -h|--help) print_usage; exit 0 ;;
    -*) echo "unknown flag: $1" >&2; exit 2 ;;
    *) [[ -z "$BRANCH" ]] && BRANCH="$1" && shift || { echo "multiple branches given" >&2; exit 2; } ;;
  esac
done

[[ -z "$BRANCH" ]] && { echo "missing <branch> argument" >&2; print_usage; exit 2; }

if [[ "$USE_COLOR" == 1 ]]; then
  C_HEAD=$'\033[1;36m'; C_WARN=$'\033[1;33m'; C_DIM=$'\033[2m'; C_OK=$'\033[1;32m'; C_END=$'\033[0m'
else
  C_HEAD=""; C_WARN=""; C_DIM=""; C_OK=""; C_END=""
fi

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

# --- Graph file resolution (prefer unified, fall back) ---
if [[ -z "$GRAPH_FILE" ]]; then
  for candidate in graphify-out-unified/graph.json graphify-out/graph.json; do
    if [[ -f "$candidate" ]]; then GRAPH_FILE="$candidate"; break; fi
  done
fi
if [[ -z "$GRAPH_FILE" || ! -f "$GRAPH_FILE" ]]; then
  echo "${C_WARN}no graph.json found (tried graphify-out-unified/, graphify-out/). Run /graphify first.${C_END}" >&2
  [[ "$REQUIRE_FRESH" == 1 ]] && exit 1 || { echo "continuing without graph — coverage will be 'UNKNOWN'"; }
fi

# --- Freshness check (Wave 2) ---
# Prefer graph-internal built_at metadata (epoch secs or ISO-8601) when present;
# fall back to file mtime. Compare against origin/main HEAD commit time.
graph_mtime=0
graph_source="none"
head_commit_time=$(git log -1 --format=%ct origin/main 2>/dev/null || git log -1 --format=%ct HEAD)
graph_age_note=""
max_age_s=$(( MAX_AGE_MIN * 60 ))

if [[ -n "$GRAPH_FILE" && -f "$GRAPH_FILE" ]]; then
  # Try graph-internal metadata first (graphify may emit built_at/generated_at in future schemas).
  built_at_raw=$(jq -r '.built_at // .generated_at // .graph.built_at // .graph.generated_at // ""' "$GRAPH_FILE" 2>/dev/null || true)
  if [[ -n "$built_at_raw" && "$built_at_raw" != "null" && "$built_at_raw" != "" ]]; then
    if [[ "$built_at_raw" =~ ^[0-9]+$ ]]; then
      graph_mtime="$built_at_raw"
      graph_source="built_at(epoch)"
    else
      # ISO-8601 → epoch. date -d is GNU-only; fall back to python if it fails.
      graph_mtime=$(date -d "$built_at_raw" +%s 2>/dev/null || python3 -c "import datetime,sys; print(int(datetime.datetime.fromisoformat(sys.argv[1].replace('Z','+00:00')).timestamp()))" "$built_at_raw" 2>/dev/null || echo 0)
      graph_source="built_at(iso)"
    fi
  fi
  if [[ "$graph_mtime" == 0 ]]; then
    graph_mtime=$(stat -c %Y "$GRAPH_FILE" 2>/dev/null || stat -f %m "$GRAPH_FILE")
    graph_source="file_mtime"
  fi

  age_s=$(( head_commit_time - graph_mtime ))
  if (( age_s > max_age_s )); then
    graph_age_note="${C_WARN}STALE (graph $((age_s/60))min behind origin/main HEAD; threshold=${MAX_AGE_MIN}min; source=${graph_source})${C_END}"
    if [[ "$REQUIRE_FRESH" == 1 ]]; then
      {
        echo "$graph_age_note"
        echo "  HEAD commit time: $(date -d "@$head_commit_time" -Iseconds 2>/dev/null || echo "@$head_commit_time")"
        echo "  graph build time: $(date -d "@$graph_mtime" -Iseconds 2>/dev/null || echo "@$graph_mtime")"
        echo "  run: /graphify --update   (or pass --max-age-min $(( (age_s/60)+5 )) to this probe)"
      } >&2
      exit 1
    fi
  elif (( age_s < 0 )); then
    graph_age_note="${C_OK}fresh (graph newer than HEAD by $(( -age_s/60 ))min; source=${graph_source})${C_END}"
  else
    graph_age_note="${C_OK}fresh ($((age_s/60))min behind HEAD; source=${graph_source})${C_END}"
  fi
fi

# --- Changed files + functions ---
base=$(git merge-base origin/main "$BRANCH" 2>/dev/null || git merge-base HEAD "$BRANCH")
changed_files=$(git diff --name-only "$base" "$BRANCH" || true)
file_count=$(echo "$changed_files" | grep -c . || true)

# Function-name extraction from diff hunks (best-effort regex — Wave 1 scaffold).
# Captures: Rust fn/pub fn, Python def, JS/TS function + arrow-function assigned, shell func.
changed_fns=$(git diff --unified=0 "$base" "$BRANCH" 2>/dev/null | awk '
  /^\+\+\+ / { file=$2; sub("^b/", "", file); next }
  /^\+/ && !/^\+\+\+/ {
    line=$0; sub("^\\+", "", line)
    if (match(line, /^[[:space:]]*(pub[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+([A-Za-z_][A-Za-z0-9_]*)/, a)) print a[3]
    else if (match(line, /^[[:space:]]*def[[:space:]]+([A-Za-z_][A-Za-z0-9_]*)/, a)) print a[1]
    else if (match(line, /^[[:space:]]*function[[:space:]]+([A-Za-z_][A-Za-z0-9_]*)/, a)) print a[1]
    else if (match(line, /^[[:space:]]*(const|let|var)[[:space:]]+([A-Za-z_][A-Za-z0-9_]*)[[:space:]]*=[[:space:]]*(async[[:space:]]*)?\(/, a)) print a[2]
    else if (match(line, /^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*)\(\)[[:space:]]*\{/, a)) print a[1]
  }
' | sort -u)
fn_count=$(echo "$changed_fns" | grep -c . || true)

# --- Header ---
echo ""
echo "${C_HEAD}═══ graphify deploy probe (Wave 1+2+3+4) ═══${C_END}"
printf "%-22s %s\n" "branch:"       "$BRANCH"
printf "%-22s %s\n" "merge-base:"   "${base:-unknown}"
printf "%-22s %s\n" "graph.json:"   "${GRAPH_FILE:-<absent>}"
printf "%-22s %s\n" "graph freshness:" "$graph_age_note"
printf "%-22s %s\n" "changed files:" "$file_count"
printf "%-22s %s\n" "changed functions:" "$fn_count"
echo ""

# --- Graph-based coverage + blast-radius (Wave 4: 2-hop BFS + god-node + communities) ---
if [[ -n "$GRAPH_FILE" && -f "$GRAPH_FILE" && "$fn_count" -gt 0 ]]; then
  echo "${C_HEAD}── function coverage + ${MAX_HOPS}-hop blast radius (W4) ──${C_END}"
  node_json=$(mktemp)
  jq -c '.nodes' "$GRAPH_FILE" > "$node_json"

  # Precompute (once) — avoid O(fn × links) scans later.
  #   adj_tmp       undirected adjacency: {id: [neighbor_id,...]}
  #   in_deg_tmp    directed in-degree:   {id: count}
  #   meta_tmp      id → {community, source_file, label}
  adj_tmp=$(mktemp)
  in_deg_tmp=$(mktemp)
  meta_tmp=$(mktemp)
  jq -c '
    reduce .links[] as $l ({};
      .[$l.source | tostring] += [$l.target] |
      .[$l.target | tostring] += [$l.source])
    | with_entries(.value |= unique)
  ' "$GRAPH_FILE" > "$adj_tmp"
  jq -c '
    reduce .links[] as $l ({}; .[$l.target | tostring] += 1)
  ' "$GRAPH_FILE" > "$in_deg_tmp"
  jq -c '
    .nodes | map({ (.id | tostring): { community: .community, source_file: .source_file, label: .label } }) | add
  ' "$GRAPH_FILE" > "$meta_tmp"

  indexed=0
  unindexed=0
  god_nodes=0
  total_blast_seed_max=0
  total_blast_seed_max_fn=""
  while IFS= read -r fn; do
    [[ -z "$fn" ]] && continue
    # Exact label match first, then norm_label.
    node=$(jq -c --arg fn "$fn" 'map(select(.label == $fn or .norm_label == $fn)) | .[0] // empty' "$node_json")
    if [[ -z "$node" || "$node" == "null" ]]; then
      unindexed=$((unindexed+1))
      printf "  %s%-40s%s %sNOT in graph%s\n" "$C_DIM" "$fn" "$C_END" "$C_WARN" "$C_END"
      continue
    fi
    indexed=$((indexed+1))
    node_id=$(echo "$node" | jq -r '.id | tostring')
    community=$(echo "$node" | jq -r '.community')
    source_file=$(echo "$node" | jq -r '.source_file')

    # Hop 1 — direct neighbours.
    hop1_ids=$(jq -c --arg id "$node_id" '.[$id] // []' "$adj_tmp")
    hop1_count=$(echo "$hop1_ids" | jq 'length')

    # Hop 2 — neighbours-of-neighbours, minus seed and hop1 (dedup via exclude set).
    hop2_count=0
    blast_ids="$hop1_ids"
    if (( MAX_HOPS >= 2 && hop1_count > 0 )); then
      hop2_ids=$(jq -c \
        --argjson h1 "$hop1_ids" \
        --arg seed "$node_id" \
        '
          ($h1 + [$seed | tonumber? // $seed]) as $exclude |
          [ $h1[] as $n | .[$n | tostring] // [] | .[] ]
          | unique
          | map(select(. as $x | $exclude | index($x) | not))
        ' "$adj_tmp")
      hop2_count=$(echo "$hop2_ids" | jq 'length')
      blast_ids=$(jq -c --argjson h1 "$hop1_ids" --argjson h2 "$hop2_ids" '$h1 + $h2 | unique' <<<'null')
    fi
    blast_total=$(echo "$blast_ids" | jq 'length')

    # In-degree — god-node if >= threshold.
    in_degree=$(jq -r --arg id "$node_id" '.[$id] // 0' "$in_deg_tmp")
    god_flag=""
    if (( in_degree >= GOD_NODE_THRESHOLD )); then
      god_flag=" ${C_WARN}☆GOD${C_END}"
      god_nodes=$((god_nodes+1))
    fi

    # Communities touched across blast set.
    communities=$(jq -r --argjson ids "$blast_ids" '
      to_entries | map(select(.key as $k | $ids | map(tostring) | index($k))) | map(.value.community) | unique | map(tostring) | join(",")
    ' "$meta_tmp")
    comm_count=$(echo -n "$communities" | awk -F',' '{print NF}')

    if (( blast_total > total_blast_seed_max )); then
      total_blast_seed_max=$blast_total
      total_blast_seed_max_fn=$fn
    fi

    printf "  %-40s community=%s  1h=%s  2h=%s  blast=%s  in-deg=%s%s\n" \
      "$fn" "$community" "$hop1_count" "$hop2_count" "$blast_total" "$in_degree" "$god_flag"
    printf "    ${C_DIM}file=%s  touches communities: %s (%s)${C_END}\n" \
      "$source_file" "${communities:-none}" "$comm_count"
  done <<< "$changed_fns"

  rm -f "$node_json" "$adj_tmp" "$in_deg_tmp" "$meta_tmp"
  echo ""
  printf "${C_OK}indexed: %s${C_END}  ${C_WARN}unindexed: %s${C_END}  ${C_WARN}god-nodes: %s${C_END}  (of %s changed functions, threshold in-deg >= %s)\n" \
    "$indexed" "$unindexed" "$god_nodes" "$fn_count" "$GOD_NODE_THRESHOLD"
  if (( total_blast_seed_max > 0 )); then
    printf "${C_DIM}max blast seed: %s (%s nodes within %s hops)${C_END}\n" \
      "$total_blast_seed_max_fn" "$total_blast_seed_max" "$MAX_HOPS"
  fi
fi

# --- Coverage matrix (Wave 3 — dynamic from graph.json) ---
# Canonical prefix list derived from project_graphify_utilization_roadmap.md Tier 1/1b.
# Each row is `prefix_to_match|label`. Substring match against node source_file in graph.
# Keep label width fixed for visual alignment in the table.
echo ""
echo "${C_HEAD}── repo coverage matrix (Wave 3 — dynamic) ──${C_END}"

coverage_rows=(
  "crates/|racecontrol/crates/"
  "racecontrol/src/|racecontrol/app/"
  "scripts/|racecontrol/scripts/"
  "tests/e2e/|racecontrol/tests/e2e/"
  "packages/shared-types/|packages/shared-types/"
  "comms-link/|comms-link/"
  "racingpoint-admin/|racingpoint-admin/"
  "racingpoint-whatsapp-bot/|racingpoint-whatsapp-bot/"
  "racingpoint-discord-bot/|racingpoint-discord-bot/"
  "racingpoint-voice/|racingpoint-voice/"
  "racingpoint-dashboard/|racingpoint-dashboard/"
)

if [[ -n "$GRAPH_FILE" && -f "$GRAPH_FILE" ]]; then
  # Extract unique source_file set once to avoid re-reading JSON per row.
  sources_tmp=$(mktemp)
  jq -r '.nodes[] | .source_file // empty' "$GRAPH_FILE" 2>/dev/null | sort -u > "$sources_tmp"
  total_sources=$(wc -l < "$sources_tmp")
  for row in "${coverage_rows[@]}"; do
    prefix="${row%%|*}"
    label="${row#*|}"
    # Count unique source files whose path contains prefix (case-insensitive, handle both / and \).
    # `|| true` because grep -c returns 1 on zero matches (pipefail would kill us).
    hit=$(grep -ci -E "(^|[/\\\\])$(echo "$prefix" | sed 's/[.[\*^$()+?{|]/\\&/g')" "$sources_tmp" 2>/dev/null | head -1 || true)
    hit=${hit:-0}
    if (( hit > 0 )); then
      printf "  %-38s INDEXED   (%s file%s)\n" "$label" "$hit" "$([[ $hit == 1 ]] || echo s)"
    else
      printf "  %-38s ${C_WARN}NOT INDEXED${C_END} (MANUAL-REVIEW if changed)\n" "$label"
    fi
  done
  printf "${C_DIM}  (total unique source files in graph: %s)${C_END}\n" "$total_sources"
  rm -f "$sources_tmp"
else
  for row in "${coverage_rows[@]}"; do
    label="${row#*|}"
    printf "  %-38s UNKNOWN   (no graph.json loaded)\n" "$label"
  done
fi

# --- Manual-review flags (Wave 3 — derived from coverage matrix misses) ---
# Build a dynamic unindexed-prefix list from the coverage rows that just missed,
# instead of the Wave 1 hardcoded `scripts/|tests/e2e/|packages/shared-types/`.
unindexed_prefixes=""
if [[ -n "$GRAPH_FILE" && -f "$GRAPH_FILE" ]]; then
  sources_tmp=$(mktemp)
  jq -r '.nodes[] | .source_file // empty' "$GRAPH_FILE" 2>/dev/null | sort -u > "$sources_tmp"
  for row in "${coverage_rows[@]}"; do
    prefix="${row%%|*}"
    hit_mr=$(grep -ci -E "(^|[/\\\\])$(echo "$prefix" | sed 's/[.[\*^$()+?{|]/\\&/g')" "$sources_tmp" 2>/dev/null | head -1 || true)
    hit_mr=${hit_mr:-0}
    (( hit_mr == 0 )) && unindexed_prefixes="${unindexed_prefixes}^${prefix}|"
  done
  rm -f "$sources_tmp"
fi
# Strip trailing |
unindexed_prefixes="${unindexed_prefixes%|}"

if [[ -n "$unindexed_prefixes" && -n "$changed_files" ]]; then
  # shellcheck disable=SC2086
  unindexed_paths=$(echo "$changed_files" | awk -v pfx="$unindexed_prefixes" 'BEGIN{n=split(pfx,arr,"|")} { for(i=1;i<=n;i++){ p=arr[i]; sub(/^\^/,"",p); if (index($0,p)==1) { print; break } } }' | head -20)
  if [[ -n "$unindexed_paths" ]]; then
    echo ""
    echo "${C_WARN}── MANUAL REVIEW (changed files fall under NOT INDEXED prefixes) ──${C_END}"
    echo "$unindexed_paths" | sed 's/^/  /'
  fi
fi

echo ""
echo "${C_DIM}Wave 1+2+3+4 — freshness (--max-age-min/--require-fresh) + dynamic coverage matrix + ${MAX_HOPS}-hop BFS blast radius + god-node flag (threshold in-deg >= ${GOD_NODE_THRESHOLD}) + per-function community-touch set. Waves 5/6/7 add: E2E cross-ref, graph-diff, symptom probe.${C_END}"
exit 0
