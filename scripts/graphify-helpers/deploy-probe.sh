#!/usr/bin/env bash
# deploy-probe.sh — graphify blast-radius probe for deploy-gate enumeration.
#
# Wave 1 scaffold + Wave 2 freshness — plan_graphify_blast_radius_20260423.md.
# Next waves: 2-hop BFS (W4), E2E cross-ref (W5), graph-diff (W6), symptom probe (W7).
#
# Usage: bash scripts/graphify-helpers/deploy-probe.sh <branch> [flags]
# Flags:
#   --require-fresh          exit 1 if graph older than HEAD by > MAX_AGE
#   --max-age-min N          stale threshold in minutes (default 60)
#   --max-hops N             BFS depth (default 1 in this wave)
#   --graph-file PATH        override default graph lookup
#   --no-color               disable ANSI colors
#   -h | --help              print usage
# Exit codes:
#   0 success    1 stale/missing graph (with --require-fresh)    2 query/arg failure

set -euo pipefail

BRANCH=""
REQUIRE_FRESH=0
MAX_HOPS=1
MAX_AGE_MIN=60
GRAPH_FILE=""
USE_COLOR=1

print_usage() { sed -n '3,17p' "$0" | sed 's/^# \{0,1\}//'; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --require-fresh) REQUIRE_FRESH=1; shift ;;
    --max-age-min) MAX_AGE_MIN="$2"; shift 2 ;;
    --max-hops) MAX_HOPS="$2"; shift 2 ;;
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
echo "${C_HEAD}═══ graphify deploy probe (Wave 1+2 scaffold) ═══${C_END}"
printf "%-22s %s\n" "branch:"       "$BRANCH"
printf "%-22s %s\n" "merge-base:"   "${base:-unknown}"
printf "%-22s %s\n" "graph.json:"   "${GRAPH_FILE:-<absent>}"
printf "%-22s %s\n" "graph freshness:" "$graph_age_note"
printf "%-22s %s\n" "changed files:" "$file_count"
printf "%-22s %s\n" "changed functions:" "$fn_count"
echo ""

# --- Graph-based coverage + blast-radius (scaffold — 1-hop only in W1) ---
if [[ -n "$GRAPH_FILE" && -f "$GRAPH_FILE" && "$fn_count" -gt 0 ]]; then
  echo "${C_HEAD}── function coverage + 1-hop neighbours ──${C_END}"
  node_json=$(mktemp)
  jq -c '.nodes' "$GRAPH_FILE" > "$node_json"
  link_json=$(mktemp)
  jq -c '.links' "$GRAPH_FILE" > "$link_json"

  indexed=0
  unindexed=0
  while IFS= read -r fn; do
    [[ -z "$fn" ]] && continue
    # Exact label match first, then norm_label, then contains on label.
    node=$(jq -c --arg fn "$fn" 'map(select(.label == $fn or .norm_label == $fn)) | .[0] // empty' "$node_json")
    if [[ -z "$node" || "$node" == "null" ]]; then
      unindexed=$((unindexed+1))
      printf "  %s%-40s%s %sNOT in graph%s\n" "$C_DIM" "$fn" "$C_END" "$C_WARN" "$C_END"
      continue
    fi
    indexed=$((indexed+1))
    node_id=$(echo "$node" | jq -r '.id')
    community=$(echo "$node" | jq -r '.community')
    source_file=$(echo "$node" | jq -r '.source_file')
    # 1-hop neighbours (both directions)
    nbr_count=$(jq --arg id "$node_id" '[ .[] | select(.source == $id or .target == $id) ] | length' "$link_json")
    printf "  %-40s community=%s  file=%s  1-hop=%s\n" "$fn" "$community" "$source_file" "$nbr_count"
  done <<< "$changed_fns"

  rm -f "$node_json" "$link_json"
  echo ""
  printf "${C_OK}indexed: %s${C_END}  ${C_WARN}unindexed: %s${C_END}  (of %s changed functions)\n" "$indexed" "$unindexed" "$fn_count"
fi

# --- Coverage matrix (W3 stub — static repo list, to be generated from roadmap) ---
echo ""
echo "${C_HEAD}── repo coverage matrix (W1 stub — static) ──${C_END}"
cat <<EOF
  racecontrol/crates/........... INDEXED (graphify-racecontrol)
  racecontrol/app/.............. INDEXED
  racecontrol/scripts/.......... NOT INDEXED  (MANUAL-REVIEW if changed)
  racecontrol/tests/e2e/........ NOT INDEXED  (MANUAL-REVIEW if changed)
  packages/shared-types/........ NOT INDEXED  (MANUAL-REVIEW if changed)
  comms-link/................... INDEXED (graphify-comms-link)
  racingpoint-admin/............ INDEXED (graphify-admin)
  racingpoint-whatsapp-bot/..... INDEXED (graphify-whatsapp)
  (Bono-only repos × 6)......... NOT INDEXED  (Tier 1b deferred per roadmap)
EOF

# --- Manual-review flags ---
unindexed_paths=$(echo "$changed_files" | awk '/^scripts\// || /^tests\/e2e\// || /^packages\/shared-types\// {print}' | head -20)
if [[ -n "$unindexed_paths" ]]; then
  echo ""
  echo "${C_WARN}── MANUAL REVIEW (unindexed paths changed) ──${C_END}"
  echo "$unindexed_paths" | sed 's/^/  /'
fi

echo ""
echo "${C_DIM}Wave 1+2 — 1-hop neighbours, freshness-via-built_at/mtime, --max-age-min + --require-fresh. Waves 3/4/5/6/7 add: dynamic coverage matrix, 2-hop BFS, E2E cross-ref, graph-diff, symptom probe.${C_END}"
exit 0
