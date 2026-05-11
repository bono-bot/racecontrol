#!/usr/bin/env bash
# v2-deploy/deploy.sh — F9 Atomic-Deploy v1 entry-point router (PACT-20260503-004)
#
# CONSTRAINT-019: V2.0 deploy of any surface MUST execute through F9 deploy.sh OR deploy.yml.
# Manual surface-only deploys are forbidden post-F9 ratify.
#
# Per spec `.planning/specs/v2-step-2.5-f9-atomic-deploy.md` (amend1 commit 73dcf5f0).
#
# Usage: bash v2-deploy/deploy.sh <surface> [flags]
# Surfaces: racecontrol | rc-agent | web | kiosk | admin | comms-link | pwa
# Flags: --canary=podN | --dry-run | --no-swaplog | --manifest=PATH | --allow-dirty | --allow-red-ci
#
# F9 v1 wraps existing per-surface deploy infra and adds:
#   - manifest write (v2-deploy/manifests/<surface>-<ts>.json)
#   - SWAPLOG.md append (audit-trail; idempotent)
#   - HALO probe trigger (advisory in v1)
#
# Exit codes: 0 = success | 1 = pre-condition fail | 2 = delegate fail | 3 = swaplog/manifest fail
# Out of scope (per spec): cross-surface transactional rollback (F9 v2) | auto-canary | blue-green

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RACECONTROL_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SWAPLOG="${SCRIPT_DIR}/SWAPLOG.md"
MANIFEST_DIR="${SCRIPT_DIR}/manifests"
LEGACY_DEPLOY="${RACECONTROL_DIR}/scripts/deploy"

# ---------- arg parsing ----------

SURFACE=""
CANARY_POD=""
DRY_RUN=0
NO_SWAPLOG=0
MANIFEST_PATH=""
ALLOW_DIRTY=0
ALLOW_RED_CI=0

while [ $# -gt 0 ]; do
  case "$1" in
    --canary=*)      CANARY_POD="${1#--canary=}";;
    --dry-run)       DRY_RUN=1;;
    --no-swaplog)    NO_SWAPLOG=1;;
    --manifest=*)    MANIFEST_PATH="${1#--manifest=}";;
    --allow-dirty)   ALLOW_DIRTY=1;;
    --allow-red-ci)  ALLOW_RED_CI=1;;
    --help|-h)
      sed -n '4,17p' "$0"
      exit 0
      ;;
    -*)
      echo "ERROR: unknown flag $1" >&2
      exit 1
      ;;
    *)
      if [ -z "$SURFACE" ]; then SURFACE="$1"; else echo "ERROR: unexpected positional arg $1" >&2; exit 1; fi
      ;;
  esac
  shift
done

if [ -z "$SURFACE" ]; then
  echo "ERROR: missing <surface>. Usage: bash v2-deploy/deploy.sh <surface> [flags]" >&2
  echo "Surfaces: racecontrol | rc-agent | web | kiosk | admin | comms-link | pwa" >&2
  exit 1
fi

case "$SURFACE" in
  racecontrol|rc-agent|web|kiosk|admin|comms-link|pwa) ;;
  *)
    echo "ERROR: invalid surface '$SURFACE'. Must be one of: racecontrol|rc-agent|web|kiosk|admin|comms-link|pwa" >&2
    exit 1
    ;;
esac

if [ "$SURFACE" != "rc-agent" ] && [ -n "$CANARY_POD" ]; then
  echo "ERROR: --canary flag is only valid for rc-agent surface" >&2
  exit 1
fi

# ---------- environment capture ----------

TS_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
TS_IST="$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST')"
GIT_SHA="$(cd "$RACECONTROL_DIR" && git rev-parse HEAD)"
GIT_BRANCH="$(cd "$RACECONTROL_DIR" && git rev-parse --abbrev-ref HEAD)"
GIT_AUTHOR="$(cd "$RACECONTROL_DIR" && git log -1 --pretty=format:'%an <%ae>')"
GIT_COMMITTER="${USER:-$(whoami)}"
CALLER_TAG="f9-deploy-sh"
[ -n "${GITHUB_ACTIONS:-}" ] && CALLER_TAG="f9-deploy-yml"

if [ -z "$MANIFEST_PATH" ]; then
  mkdir -p "$MANIFEST_DIR"
  MANIFEST_PATH="${MANIFEST_DIR}/${SURFACE}-${TS_UTC//:/}.json"
fi

# ---------- pre-condition gates ----------

check_git_clean() {
  if [ "$ALLOW_DIRTY" -eq 1 ]; then
    echo "[F9] PRE-CONDITION: git_clean SKIPPED (--allow-dirty)"
    return 0
  fi
  cd "$RACECONTROL_DIR"
  if [ -n "$(git status --porcelain | grep -v '^??' | grep -v '\.planning/board-state' | grep -v 'Cargo.lock' | head -1)" ]; then
    echo "[F9] PRE-CONDITION FAIL: git working tree dirty (use --allow-dirty to override)" >&2
    git status --short | head -10 >&2
    return 1
  fi
  echo "[F9] PRE-CONDITION OK: git_clean"
  return 0
}

check_constraint_019() {
  local pact_charter="${RACECONTROL_DIR}/../comms-link/PACT-CHARTER.md"
  if [ ! -f "$pact_charter" ]; then
    echo "[F9] PRE-CONDITION SKIP: PACT-CHARTER.md not found at $pact_charter" >&2
    return 0
  fi
  if ! grep -q "CONSTRAINT-019" "$pact_charter"; then
    echo "[F9] PRE-CONDITION FAIL: CONSTRAINT-019 binding-text not present in PACT-CHARTER.md" >&2
    return 1
  fi
  echo "[F9] PRE-CONDITION OK: constraint_019_binding"
  return 0
}

PRE_GIT_CLEAN=0
PRE_CI_GREEN=0
PRE_CONSTRAINT_019=0

check_git_clean && PRE_GIT_CLEAN=1
# CI-green check: defer to caller (env var set by CI on green); --allow-red-ci bypasses
if [ "$ALLOW_RED_CI" -eq 1 ] || [ "${F9_CI_GREEN:-0}" = "1" ]; then PRE_CI_GREEN=1; fi
check_constraint_019 && PRE_CONSTRAINT_019=1

if [ "$PRE_GIT_CLEAN" -eq 0 ] || [ "$PRE_CONSTRAINT_019" -eq 0 ]; then
  echo "[F9] ABORT: pre-conditions failed. Use --allow-dirty / --allow-red-ci flags only when intentional." >&2
  exit 1
fi

# ---------- delegate routing ----------

DELEGATE_CMD=""
DELEGATE_EXIT=0

case "$SURFACE" in
  racecontrol)
    DELEGATE_CMD="bash ${LEGACY_DEPLOY}/deploy.sh racecontrol"
    ;;
  rc-agent)
    if [ -n "$CANARY_POD" ]; then
      DELEGATE_CMD="bash ${LEGACY_DEPLOY}/deploy-pod.sh ${CANARY_POD}"
    else
      DELEGATE_CMD="bash ${LEGACY_DEPLOY}/deploy-all-pods.sh"
    fi
    ;;
  web|kiosk|admin)
    DELEGATE_CMD="bash ${LEGACY_DEPLOY}/deploy-nextjs.sh ${SURFACE}"
    ;;
  comms-link)
    # Bono VPS deploy via comms-link relay (canonical V2 transport)
    DELEGATE_CMD="curl -s -X POST http://localhost:8766/relay/exec/run -H 'Content-Type: application/json' -d '{\"command\":\"git_pull\",\"reason\":\"F9 v1 deploy comms-link\"}'"
    ;;
  pwa)
    # PWA cloud rebuild via comms-link relay
    DELEGATE_CMD="curl -s -X POST http://localhost:8766/relay/chain/run -H 'Content-Type: application/json' -d '{\"template\":\"deploy-bono\"}'"
    ;;
esac

# ---------- execution ----------

DELEGATE_START="$(date +%s)"
echo ""
echo "================================================================================"
echo "[F9] DEPLOY START: surface=${SURFACE}  ts=${TS_IST}  git_sha=${GIT_SHA:0:12}  caller=${CALLER_TAG}"
[ "$DRY_RUN" -eq 1 ] && echo "[F9] DRY-RUN: would execute: $DELEGATE_CMD"
echo "================================================================================"
echo ""

if [ "$DRY_RUN" -eq 0 ]; then
  echo "[F9] DELEGATE: $DELEGATE_CMD"
  if eval "$DELEGATE_CMD"; then
    DELEGATE_EXIT=0
    echo "[F9] DELEGATE OK"
  else
    DELEGATE_EXIT=$?
    echo "[F9] DELEGATE FAIL exit=$DELEGATE_EXIT" >&2
  fi
else
  DELEGATE_EXIT=0
  echo "[F9] DRY-RUN skip"
fi

DELEGATE_END="$(date +%s)"
DURATION_S=$((DELEGATE_END - DELEGATE_START))

# ---------- manifest write ----------

write_manifest() {
  cat > "$MANIFEST_PATH" <<MANIFEST_END
{
  "pact": "PACT-20260503-004",
  "f9_version": "v1",
  "surface": "${SURFACE}",
  "git_sha": "${GIT_SHA}",
  "git_branch": "${GIT_BRANCH}",
  "git_author": "${GIT_AUTHOR}",
  "git_committer": "${GIT_COMMITTER}",
  "ts_utc": "${TS_UTC}",
  "ts_ist": "${TS_IST}",
  "binary_hash": "n/a-v1",
  "caller": "${CALLER_TAG}",
  "pre_conditions": {
    "git_clean": $([ "$PRE_GIT_CLEAN" -eq 1 ] && echo true || echo false),
    "ci_green": $([ "$PRE_CI_GREEN" -eq 1 ] && echo true || echo false),
    "constraint_019_binding": $([ "$PRE_CONSTRAINT_019" -eq 1 ] && echo true || echo false)
  },
  "delegate": {
    "cmd": $(printf '%s' "$DELEGATE_CMD" | python -c 'import sys,json; print(json.dumps(sys.stdin.read()))' 2>/dev/null || printf '"%s"' "$DELEGATE_CMD"),
    "exit_code": ${DELEGATE_EXIT},
    "duration_s": ${DURATION_S}
  },
  "swaplog_appended": $([ "$NO_SWAPLOG" -eq 0 ] && echo true || echo false),
  "canary_pod": "${CANARY_POD}",
  "dry_run": $([ "$DRY_RUN" -eq 1 ] && echo true || echo false)
}
MANIFEST_END
  echo "[F9] MANIFEST: $MANIFEST_PATH"
}

write_manifest || { echo "[F9] WARN: manifest write failed" >&2; exit 3; }

# ---------- SWAPLOG append ----------

if [ "$NO_SWAPLOG" -eq 0 ]; then
  REL_MANIFEST="$(realpath --relative-to="$RACECONTROL_DIR" "$MANIFEST_PATH" 2>/dev/null || echo "$MANIFEST_PATH")"
  NOTES=""
  [ "$DRY_RUN" -eq 1 ] && NOTES="dry-run"
  [ -n "$CANARY_POD" ] && NOTES="${NOTES:+$NOTES; }canary=${CANARY_POD}"
  [ "$DELEGATE_EXIT" -ne 0 ] && NOTES="${NOTES:+$NOTES; }delegate-failed"

  ROW="| ${TS_UTC} | ${TS_IST} | ${SURFACE} | ${GIT_SHA} | ${GIT_AUTHOR} | ${REL_MANIFEST} | ${DELEGATE_EXIT} | ${DURATION_S} | ${NOTES} |"

  # Idempotent append: skip if exact row already present (handles retry-after-failure cases)
  if [ -f "$SWAPLOG" ] && grep -qF "$ROW" "$SWAPLOG"; then
    echo "[F9] SWAPLOG: row already present (idempotent skip)"
  else
    echo "$ROW" >> "$SWAPLOG"
    echo "[F9] SWAPLOG: appended row"
  fi
fi

# ---------- HALO probe trigger (advisory in v1) ----------

if [ -x "${SCRIPT_DIR}/halo-probe-deploy-source-attribution.sh" ]; then
  echo ""
  echo "[F9] HALO PROBE: deploy-source-attribution (advisory)"
  bash "${SCRIPT_DIR}/halo-probe-deploy-source-attribution.sh" --surface="$SURFACE" || {
    echo "[F9] HALO PROBE: advisory finding (non-blocking in v1; becomes fail-CLOSED in Phase 3)"
  }
fi

# ---------- exit ----------

if [ "$DELEGATE_EXIT" -eq 0 ]; then
  echo ""
  echo "[F9] DEPLOY COMPLETE: surface=${SURFACE} duration=${DURATION_S}s manifest=${MANIFEST_PATH}"
  exit 0
else
  echo ""
  echo "[F9] DEPLOY FAILED: surface=${SURFACE} delegate_exit=${DELEGATE_EXIT}" >&2
  exit 2
fi
