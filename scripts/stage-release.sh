#!/bin/bash
# =============================================================================
# stage-release.sh — Build, verify, and stage all binaries for deployment
#
# This script closes the critical gap between "code committed" and "binary
# ready to deploy". It automates:
#   1. Touch build.rs to force GIT_HASH refresh (prevents cargo caching stale hashes)
#   2. cargo build --release for all deployment crates
#   3. Copy binaries to deploy-staging/
#   4. Compute SHA256 hashes
#   5. Generate release-manifest.toml with all metadata
#   6. Staleness detection (warns if staging is behind HEAD)
#
# Usage:
#   ./scripts/stage-release.sh              # build + stage all binaries
#   ./scripts/stage-release.sh --agent-only # build + stage rc-agent only
#   ./scripts/stage-release.sh --server-only # build + stage racecontrol only
#   ./scripts/stage-release.sh --check      # check if staging is stale (no build)
# =============================================================================

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
STAGING_DIR="${STAGING_DIR:-$HOME/racingpoint/deploy-staging}"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; }
info() { echo -e "  ${YELLOW}...${NC}   $1"; }

MODE="${1:---all}"

echo "=========================================="
echo "  Stage Release"
echo "  Repo:    ${REPO_DIR}"
echo "  Staging: ${STAGING_DIR}"
echo "=========================================="
echo ""

# ─── Security pre-flight (SEC-GATE-01) ───────────────────────────────
# Run security check before building — blocks staging if security regresses.
COMMS_ROOT="${REPO_DIR}/../comms-link"
if [ -f "${COMMS_ROOT}/test/security-check.js" ]; then
    info "Running security pre-flight (SEC-GATE-01)..."
    if node "${COMMS_ROOT}/test/security-check.js" 2>&1 | tail -5; then
        pass "Security pre-flight passed"
    else
        fail "Security pre-flight FAILED — fix security regressions before staging"
        echo -e "  ${RED}Run: node ${COMMS_ROOT}/test/security-check.js${NC} for details"
        exit 1
    fi
else
    echo -e "  ${YELLOW}!!${NC}    security-check.js not found — skipping pre-flight"
fi

# ─── Validate environment ────────────────────────────────────────────
cd "$REPO_DIR"

if [ ! -f "Cargo.toml" ]; then
    fail "Not in racecontrol repo root: ${REPO_DIR}"
    exit 1
fi

GIT_HASH=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
GIT_DIRTY=$(git diff --quiet 2>/dev/null && echo "" || echo "-dirty")
FULL_VERSION="${GIT_HASH}${GIT_DIRTY}"
echo -e "  ${CYAN}>>>${NC}   Git HEAD: ${FULL_VERSION}"

if [ -n "$GIT_DIRTY" ]; then
    echo -e "  ${YELLOW}!!${NC}    Working tree has uncommitted changes"
fi

# ─── Check-only mode ─────────────────────────────────────────────────
if [ "$MODE" = "--check" ]; then
    echo ""
    echo "─── Staleness Check ───"
    STALE=0

    for CRATE_BIN in rc-agent:rc-agent.exe racecontrol:racecontrol.exe rc-sentry:rc-sentry.exe; do
        CRATE="${CRATE_BIN%%:*}"
        BINARY="${CRATE_BIN##*:}"
        STAGED="${STAGING_DIR}/${BINARY}"

        if [ ! -f "$STAGED" ]; then
            fail "${BINARY}: NOT STAGED"
            STALE=1
            continue
        fi

        BINARY_MTIME=$(stat -c%Y "$STAGED" 2>/dev/null || echo "0")
        LATEST_COMMIT=$(git log -1 --format=%ct -- "crates/${CRATE}/" 2>/dev/null || echo "0")

        if [ "$LATEST_COMMIT" -gt "$BINARY_MTIME" ] 2>/dev/null; then
            COMMIT_AGO=$(( ($(date +%s) - LATEST_COMMIT) / 3600 ))
            STAGE_AGO=$(( ($(date +%s) - BINARY_MTIME) / 3600 ))
            fail "${BINARY}: STALE (staged ${STAGE_AGO}h ago, code changed ${COMMIT_AGO}h ago)"
            STALE=1
        else
            pass "${BINARY}: up to date"
        fi
    done

    if [ "$STALE" -gt 0 ]; then
        echo ""
        echo -e "${RED}Staging is stale. Run: ./scripts/stage-release.sh${NC}"
        exit 1
    else
        echo ""
        echo -e "${GREEN}All staged binaries are up to date.${NC}"
        exit 0
    fi
fi

# ─── Determine which crates to build ─────────────────────────────────
BUILD_AGENT=true
BUILD_SERVER=true
BUILD_SENTRY=true

case "$MODE" in
    --agent-only)  BUILD_SERVER=false; BUILD_SENTRY=false ;;
    --server-only) BUILD_AGENT=false;  BUILD_SENTRY=false ;;
    --sentry-only) BUILD_AGENT=false;  BUILD_SERVER=false ;;
    --all) ;; # build everything
    *) echo "Unknown option: $MODE"; echo "Usage: $0 [--all|--agent-only|--server-only|--sentry-only|--check]"; exit 1 ;;
esac

# ─── Step 1: Touch build.rs to force GIT_HASH refresh ────────────────
info "Touching build.rs files to force GIT_HASH refresh..."
$BUILD_AGENT  && touch crates/rc-agent/build.rs
$BUILD_SERVER && touch crates/racecontrol/build.rs
$BUILD_SENTRY && touch crates/rc-sentry/build.rs 2>/dev/null || true
pass "build.rs files touched"

# ─── Step 2: Build release binaries ──────────────────────────────────
BUILD_TARGETS=""
$BUILD_AGENT  && BUILD_TARGETS="$BUILD_TARGETS --bin rc-agent"
$BUILD_SERVER && BUILD_TARGETS="$BUILD_TARGETS --bin racecontrol"
$BUILD_SENTRY && BUILD_TARGETS="$BUILD_TARGETS --bin rc-sentry"

info "Building release binaries (this may take a few minutes)..."
echo "  cargo build --release${BUILD_TARGETS}"
if ! cargo build --release $BUILD_TARGETS 2>&1 | tail -5; then
    fail "Build failed! Fix compilation errors before staging."
    exit 1
fi
pass "Build completed"

# ─── Step 3: Copy to staging ─────────────────────────────────────────
mkdir -p "$STAGING_DIR"
info "Copying binaries to staging..."

copy_and_hash() {
    local BINARY="$1"
    local SOURCE="${REPO_DIR}/target/release/${BINARY}"
    local DEST="${STAGING_DIR}/${BINARY}"

    if [ ! -f "$SOURCE" ]; then
        fail "${BINARY}: not found in target/release/"
        return 1
    fi

    cp "$SOURCE" "$DEST"
    local SIZE=$(stat -c%s "$DEST" 2>/dev/null || wc -c < "$DEST" 2>/dev/null || echo "0")
    local HASH=$(sha256sum "$DEST" 2>/dev/null | awk '{print $1}' || certutil -hashfile "$DEST" SHA256 2>/dev/null | grep -v hash | grep -v Cert | tr -d '[:space:]' || echo "unknown")

    pass "${BINARY}: staged (${SIZE} bytes, sha256=${HASH:0:16}...)"
    echo "$HASH"
}

AGENT_HASH=""
SERVER_HASH=""
SENTRY_HASH=""

if $BUILD_AGENT; then
    AGENT_HASH=$(copy_and_hash "rc-agent.exe") || exit 1
fi
if $BUILD_SERVER; then
    SERVER_HASH=$(copy_and_hash "racecontrol.exe") || exit 1
fi
if $BUILD_SENTRY; then
    SENTRY_HASH=$(copy_and_hash "rc-sentry.exe") || exit 1
fi

# ─── Step 4: Generate release manifest ───────────────────────────────
MANIFEST="${STAGING_DIR}/release-manifest.toml"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

info "Writing release manifest..."
cat > "$MANIFEST" << EOF
# Release manifest — generated by stage-release.sh
# DO NOT EDIT MANUALLY — regenerate with ./scripts/stage-release.sh

[release]
git_commit = "${GIT_HASH}"
git_dirty = $([ -n "$GIT_DIRTY" ] && echo "true" || echo "false")
timestamp = "${TIMESTAMP}"
staged_by = "$(whoami)@$(hostname)"

[hashes]
rc_agent_sha256 = "${AGENT_HASH}"
racecontrol_sha256 = "${SERVER_HASH}"
rc_sentry_sha256 = "${SENTRY_HASH}"

[sizes]
rc_agent = $(stat -c%s "${STAGING_DIR}/rc-agent.exe" 2>/dev/null || echo "0")
racecontrol = $(stat -c%s "${STAGING_DIR}/racecontrol.exe" 2>/dev/null || echo "0")
rc_sentry = $(stat -c%s "${STAGING_DIR}/rc-sentry.exe" 2>/dev/null || echo "0")
EOF

pass "Release manifest written: ${MANIFEST}"

# ─── Step 5: Sign manifest with SHA256 ────────────────────────────────
MANIFEST_HASH=$(sha256sum "$MANIFEST" | cut -d' ' -f1)
echo "${MANIFEST_HASH}  release-manifest.toml" > "${MANIFEST}.sha256"
pass "Manifest signed: ${MANIFEST}.sha256 (${MANIFEST_HASH:0:16}...)"

# ─── Summary ─────────────────────────────────────────────────────────
echo ""
echo "=========================================="
echo -e "${GREEN}Staging complete!${NC}"
echo ""
echo "  Git commit:  ${FULL_VERSION}"
echo "  Manifest:    ${MANIFEST}"
echo ""
echo "  Next steps:"
echo "    ./scripts/deploy-pod.sh all         # deploy rc-agent to all pods"
echo "    ./scripts/deploy-pod.sh pod-8       # deploy to canary pod first"
echo "    ./scripts/deploy-server.sh          # deploy racecontrol to server"
echo "=========================================="
