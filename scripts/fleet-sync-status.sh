#!/bin/bash
# =============================================================================
# fleet-sync-status.sh — Unified fleet sync status across all repos & targets
#
# Answers: "What's committed, what's built, what's deployed, what's missing?"
#
# Checks:
#   1. All git repos: branch, unpushed commits, dirty state
#   2. Staged build vs HEAD: is the binary stale?
#   3. All 13 deploy targets: what build_id are they running?
#   4. Frontend apps: are Next.js builds stale?
#   5. Bono VPS: is cloud in sync?
#   6. Comms-link: is the relay daemon current?
#
# Usage:
#   bash scripts/fleet-sync-status.sh           # full status
#   bash scripts/fleet-sync-status.sh --quick    # repos + build only (no network)
#   bash scripts/fleet-sync-status.sh --repos    # repos only
#   bash scripts/fleet-sync-status.sh --fleet    # deployed builds only
#   bash scripts/fleet-sync-status.sh --json     # JSON output for automation
# =============================================================================

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
RP_DIR=$(cd "${REPO_DIR}/.." && pwd)
STAGING_DIR="${STAGING_DIR:-$HOME/racingpoint/deploy-staging}"
MANIFEST="${STAGING_DIR}/release-manifest.toml"

# Colors
GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; DIM='\033[2m'; NC='\033[0m'

MODE="${1:---full}"
JSON_OUTPUT=""
TOTAL_ISSUES=0
TOTAL_WARNINGS=0

# ─── Helpers ─────────────────────────────────────────────────────────

issue()   { echo -e "  ${RED}!!${NC}  $1"; TOTAL_ISSUES=$((TOTAL_ISSUES+1)); }
warn()    { echo -e "  ${YELLOW}>>>${NC} $1"; TOTAL_WARNINGS=$((TOTAL_WARNINGS+1)); }
ok()      { echo -e "  ${GREEN}OK${NC}  $1"; }
info()    { echo -e "  ${DIM}--${NC}  $1"; }
section() { echo ""; echo -e "${BOLD}═══ $1 ═══${NC}"; }

timestamp_ist() {
    # Standing rule: never use TZ=Asia/Kolkata in Git Bash (silently fails)
    python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%d %H:%M IST'))" 2>/dev/null || date -u "+%Y-%m-%d %H:%M UTC"
}

# ─── Pod IP map (matches deploy-pod.sh) ──────────────────────────────

pod_ip() {
    case "$1" in
        1) echo "192.168.31.89" ;; 2) echo "192.168.31.33" ;;
        3) echo "192.168.31.28" ;; 4) echo "192.168.31.88" ;;
        5) echo "192.168.31.86" ;; 6) echo "192.168.31.87" ;;
        7) echo "192.168.31.38" ;; 8) echo "192.168.31.91" ;;
        *) echo "" ;;
    esac
}

SERVER_IP="192.168.31.23"
SERVER_TS="100.125.108.37"
BONO_VPS="100.70.177.44"
POS_IP="192.168.31.20"
SENTRY_PORT=8091

echo "=========================================="
echo "  Fleet Sync Status"
echo "  $(timestamp_ist)"
echo "=========================================="

# ═══════════════════════════════════════════════════════════════════════
# SECTION 1: GIT REPOS
# ═══════════════════════════════════════════════════════════════════════

if [ "$MODE" != "--fleet" ]; then
    section "GIT REPOS"

    REPOS=(
        "racecontrol:Rust monorepo (server + agents)"
        "comms-link:James<->Bono relay"
        "deploy-staging:Build artifacts & manifests"
        "whatsapp-bot:WhatsApp notification bot"
        "racingpoint-admin:Admin dashboard (standalone)"
    )

    TOTAL_UNPUSHED=0

    printf "  %-20s %-8s %-10s %-10s %-8s %s\n" "REPO" "BRANCH" "HEAD" "UNPUSHED" "DIRTY" "STATUS"
    printf "  %-20s %-8s %-10s %-10s %-8s %s\n" "----" "------" "----" "--------" "-----" "------"

    for entry in "${REPOS[@]}"; do
        REPO_NAME="${entry%%:*}"
        REPO_DESC="${entry##*:}"
        REPO_PATH="${RP_DIR}/${REPO_NAME}"

        if [ ! -d "${REPO_PATH}/.git" ]; then
            printf "  %-20s %s\n" "$REPO_NAME" "NOT A GIT REPO"
            continue
        fi

        BRANCH=$(git -C "$REPO_PATH" branch --show-current 2>/dev/null || echo "?")
        HEAD=$(git -C "$REPO_PATH" rev-parse --short HEAD 2>/dev/null || echo "?")
        DIRTY=$(git -C "$REPO_PATH" diff --quiet 2>/dev/null && echo "" || echo "YES")
        STAGED=$(git -C "$REPO_PATH" diff --cached --quiet 2>/dev/null && echo "" || echo "YES")

        # Count unpushed commits (handle no upstream gracefully)
        UNPUSHED=$(git -C "$REPO_PATH" rev-list --count '@{upstream}..HEAD' 2>/dev/null || echo "?")

        STATUS_ICON="${GREEN}synced${NC}"
        if [ "$UNPUSHED" != "0" ] && [ "$UNPUSHED" != "?" ]; then
            STATUS_ICON="${RED}PUSH NEEDED${NC}"
            TOTAL_UNPUSHED=$((TOTAL_UNPUSHED + UNPUSHED))
        fi
        if [ -n "$DIRTY" ]; then
            STATUS_ICON="${YELLOW}uncommitted${NC}"
        fi

        DIRTY_SHOW="${DIRTY:-no}"
        printf "  %-20s %-8s %-10s %-10s %-8s " "$REPO_NAME" "$BRANCH" "$HEAD" "$UNPUSHED" "$DIRTY_SHOW"
        echo -e "$STATUS_ICON"
    done

    echo ""
    if [ "$TOTAL_UNPUSHED" -gt 0 ]; then
        issue "${TOTAL_UNPUSHED} total unpushed commits across repos — Bono cannot see these"
    else
        ok "All repos synced with remotes"
    fi
fi

# ═══════════════════════════════════════════════════════════════════════
# SECTION 2: BUILD STALENESS
# ═══════════════════════════════════════════════════════════════════════

if [ "$MODE" != "--fleet" ]; then
    section "BUILD STATUS"

    RC_HEAD=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown")

    if [ -f "$MANIFEST" ]; then
        MANIFEST_HASH=$(grep 'git_hash' "$MANIFEST" 2>/dev/null | head -1 | sed 's/.*= *"//;s/".*//' || echo "unknown")
        # Also check the newer format (git_commit)
        if [ "$MANIFEST_HASH" = "unknown" ]; then
            MANIFEST_HASH=$(grep 'git_commit' "$MANIFEST" 2>/dev/null | head -1 | sed 's/.*= *"//;s/".*//' || echo "unknown")
        fi
        MANIFEST_TIME=$(grep 'timestamp' "$MANIFEST" 2>/dev/null | head -1 | sed 's/.*= *"//;s/".*//' || echo "?")

        echo "  Repo HEAD:       ${RC_HEAD}"
        echo "  Staged build:    ${MANIFEST_HASH} (${MANIFEST_TIME})"

        if [ "$MANIFEST_HASH" = "$RC_HEAD" ]; then
            ok "Staged build matches HEAD"
        else
            # Check if the difference is code or just docs
            CODE_CHANGES=$(git -C "$REPO_DIR" log --oneline "${MANIFEST_HASH}..${RC_HEAD}" -- 'crates/' 'kiosk/' 'web/' 'apps/' 2>/dev/null | wc -l || echo "?")
            if [ "$CODE_CHANGES" -gt 0 ]; then
                issue "Staged build STALE — ${CODE_CHANGES} code commits since last build"
                echo -e "       ${DIM}Run: bash scripts/stage-release.sh${NC}"
            else
                warn "HEAD ahead of staged build but ${CODE_CHANGES} code changes (docs/config only)"
            fi
        fi

        # Check individual binary staleness
        for BINARY in rc-agent racecontrol rc-sentry; do
            STAGED_BIN="${STAGING_DIR}/${BINARY}.exe"
            if [ -f "$STAGED_BIN" ]; then
                SIZE=$(stat -c%s "$STAGED_BIN" 2>/dev/null || echo "?")
                info "${BINARY}.exe staged ($(echo "$SIZE" | awk '{printf "%.1f MB", $1/1048576}'))"
            else
                warn "${BINARY}.exe NOT staged"
            fi
        done
    else
        issue "No release-manifest.toml — run: bash scripts/stage-release.sh"
    fi
fi

# ═══════════════════════════════════════════════════════════════════════
# SECTION 3: DEPLOYED BUILDS (network required)
# ═══════════════════════════════════════════════════════════════════════

if [ "$MODE" = "--full" ] || [ "$MODE" = "--fleet" ] || [ "$MODE" = "--json" ]; then
    section "DEPLOYED BUILDS"

    # Get the "expected" build from manifest
    EXPECTED=""
    if [ -f "$MANIFEST" ]; then
        EXPECTED=$(grep 'git_hash' "$MANIFEST" 2>/dev/null | head -1 | sed 's/.*= *"//;s/".*//' || echo "")
        if [ -z "$EXPECTED" ]; then
            EXPECTED=$(grep 'git_commit' "$MANIFEST" 2>/dev/null | head -1 | sed 's/.*= *"//;s/".*//' || echo "")
        fi
    fi

    printf "  %-12s %-12s %-10s %-8s %s\n" "TARGET" "BUILD_ID" "SERVICE" "STATUS" "DELTA"
    printf "  %-12s %-12s %-10s %-8s %s\n" "------" "--------" "-------" "------" "-----"

    # Windows Git Bash doesn't support grep -P — use sed instead
    extract_build_id() {
        sed -n 's/.*"build_id":"\([^"]*\)".*/\1/p' | head -1
    }

    get_build_id() {
        local URL="$1"
        local TIMEOUT="${2:-5}"
        local BODY
        BODY=$(curl -s --max-time "$TIMEOUT" "$URL" 2>/dev/null || echo "")
        echo "$BODY" | extract_build_id
    }

    check_target() {
        local NAME="$1"
        local BUILD_ID="$2"
        local SERVICE="$3"

        if [ -z "$BUILD_ID" ]; then
            printf "  %-12s %-12s %-10s " "$NAME" "---" "$SERVICE"
            echo -e "${RED}OFFLINE${NC}"
            TOTAL_ISSUES=$((TOTAL_ISSUES+1))
            return
        fi

        local STATUS="${GREEN}OK${NC}"
        local DELTA=""

        if [ -n "$EXPECTED" ] && [ "$BUILD_ID" != "$EXPECTED" ]; then
            STATUS="${YELLOW}BEHIND${NC}"
            # Count commits behind
            local BEHIND
            BEHIND=$(git -C "$REPO_DIR" rev-list --count "${BUILD_ID}..${EXPECTED}" 2>/dev/null || echo "?")
            DELTA="${BEHIND} commits"
            TOTAL_WARNINGS=$((TOTAL_WARNINGS+1))
        fi

        printf "  %-12s %-12s %-10s " "$NAME" "$BUILD_ID" "$SERVICE"
        echo -e "${STATUS}  ${DIM}${DELTA}${NC}"
    }

    # ─── Server (.23) ────────────────────────────────────────────────
    SERVER_BUILD=$(get_build_id "http://${SERVER_IP}:8080/api/v1/health")
    check_target "Server .23" "$SERVER_BUILD" "racecontrol"

    # ─── Pods 1-8 (via fleet health endpoint for efficiency) ─────────
    FLEET_DATA=$(curl -s --max-time 8 "http://${SERVER_IP}:8080/api/v1/fleet/health" 2>/dev/null || echo "[]")

    for POD_NUM in $(seq 1 8); do
        POD_BUILD=$(echo "$FLEET_DATA" | sed -n "s/.*\"pod_number\":${POD_NUM}[^}]*\"build_id\":\"\([^\"]*\)\".*/\1/p" | head -1)
        POD_WS=$(echo "$FLEET_DATA" | sed -n "s/.*\"pod_number\":${POD_NUM}[^}]*\"ws_connected\":\(true\|false\).*/\1/p" | head -1)

        if [ -z "$POD_BUILD" ] && [ -n "$POD_WS" ] && [ "$POD_WS" = "false" ]; then
            # Pod known but disconnected — try direct
            POD_BUILD=$(get_build_id "http://$(pod_ip $POD_NUM):8090/health" 3)
        fi

        check_target "Pod $POD_NUM" "$POD_BUILD" "rc-agent"
    done

    # ─── POS PC (.20) ────────────────────────────────────────────────
    # POS runs kiosk browser, not rc-agent — check if reachable
    POS_PING=$(ping -c 1 -W 2 "$POS_IP" > /dev/null 2>&1 && echo "UP" || echo "DOWN")
    if [ "$POS_PING" = "UP" ]; then
        POS_KIOSK=$(curl -s --max-time 3 "http://${POS_IP}:3300/api/health" 2>/dev/null | grep -o '"status":"ok"' || echo "")
        if [ -n "$POS_KIOSK" ]; then
            printf "  %-12s %-12s %-10s " "POS .20" "n/a" "kiosk"
            echo -e "${GREEN}OK${NC}  ${DIM}(browser)${NC}"
        else
            printf "  %-12s %-12s %-10s " "POS .20" "n/a" "kiosk"
            echo -e "${YELLOW}REACHABLE${NC}  ${DIM}kiosk not responding${NC}"
            TOTAL_WARNINGS=$((TOTAL_WARNINGS+1))
        fi
    else
        printf "  %-12s %-12s %-10s " "POS .20" "---" "kiosk"
        echo -e "${RED}OFFLINE${NC}"
        TOTAL_ISSUES=$((TOTAL_ISSUES+1))
    fi

    # ─── Bono VPS (cloud) ────────────────────────────────────────────
    BONO_BUILD=$(ssh -o ConnectTimeout=5 -o BatchMode=yes "root@${BONO_VPS}" "curl -s --max-time 5 http://localhost:8080/api/v1/health 2>/dev/null" 2>/dev/null | extract_build_id)
    check_target "Bono VPS" "$BONO_BUILD" "racecontrol"

    # ─── Bono VPS git HEAD (is code pushed?) ─────────────────────────
    BONO_HEAD=$(ssh -o ConnectTimeout=5 -o BatchMode=yes "root@${BONO_VPS}" "cd /root/racingpoint/racecontrol && git rev-parse --short HEAD 2>/dev/null" 2>/dev/null || echo "")
    if [ -n "$BONO_HEAD" ]; then
        LOCAL_HEAD=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "")
        if [ "$BONO_HEAD" != "$LOCAL_HEAD" ]; then
            warn "Bono VPS git HEAD: ${BONO_HEAD} vs local: ${LOCAL_HEAD} — code not synced"
        else
            info "Bono VPS git HEAD matches local (${BONO_HEAD})"
        fi
    fi
fi

# ═══════════════════════════════════════════════════════════════════════
# SECTION 4: FRONTEND STALENESS
# ═══════════════════════════════════════════════════════════════════════

if [ "$MODE" = "--full" ] || [ "$MODE" = "--quick" ]; then
    section "FRONTEND APPS"

    # kiosk + web are inside racecontrol; admin is a separate repo
    ADMIN_DIR="${RP_DIR}/racingpoint-admin"
    for APP_ENTRY in "kiosk:3300:${REPO_DIR}/kiosk" "web:3200:${REPO_DIR}/web" "admin:3201:${ADMIN_DIR}"; do
        APP_NAME="${APP_ENTRY%%:*}"
        APP_PORT=$(echo "$APP_ENTRY" | cut -d: -f2)
        APP_PATH=$(echo "$APP_ENTRY" | cut -d: -f3-)
        APP_DIR="$APP_NAME"

        if [ ! -d "$APP_PATH" ]; then
            info "${APP_NAME}: directory not found"
            continue
        fi

        # Determine git root for this app
        if [ -d "${APP_PATH}/.git" ]; then
            APP_GIT_ROOT="$APP_PATH"
            APP_GIT_PATH="."
        else
            APP_GIT_ROOT="$REPO_DIR"
            APP_GIT_PATH="${APP_DIR}/"
        fi

        # Last code commit to this app
        LAST_CODE_COMMIT=$(git -C "$APP_GIT_ROOT" log -1 --format="%h %ar" -- "${APP_GIT_PATH}" 2>/dev/null || echo "? ?")
        LAST_CODE_HASH=$(echo "$LAST_CODE_COMMIT" | awk '{print $1}')
        LAST_CODE_AGO=$(echo "$LAST_CODE_COMMIT" | cut -d' ' -f2-)

        # Last build (check .next/BUILD_ID if it exists)
        NEXT_BUILD="${APP_PATH}/.next/BUILD_ID"
        if [ -f "$NEXT_BUILD" ]; then
            BUILD_AGO=$(stat -c%Y "$NEXT_BUILD" 2>/dev/null || echo "0")
            NOW=$(date +%s)
            HOURS_STALE=$(( (NOW - BUILD_AGO) / 3600 ))

            LAST_CODE_TIME=$(git -C "$APP_GIT_ROOT" log -1 --format="%ct" -- "${APP_GIT_PATH}" 2>/dev/null || echo "0")
            if [ "$LAST_CODE_TIME" -gt "$BUILD_AGO" ] 2>/dev/null; then
                issue "${APP_NAME} :${APP_PORT} — BUILD STALE (code changed ${LAST_CODE_AGO}, build ${HOURS_STALE}h old)"
                echo -e "       ${DIM}Rebuild: cd ${APP_DIR} && npm run build${NC}"
            else
                ok "${APP_NAME} :${APP_PORT} — build up to date (last code: ${LAST_CODE_AGO})"
            fi
        else
            warn "${APP_NAME} :${APP_PORT} — no .next/BUILD_ID (never built or standalone deploy)"
            info "  Last code change: ${LAST_CODE_HASH} (${LAST_CODE_AGO})"
        fi
    done
fi

# ═══════════════════════════════════════════════════════════════════════
# SECTION 5: COMMS-LINK STATUS
# ═══════════════════════════════════════════════════════════════════════

if [ "$MODE" = "--full" ]; then
    section "COMMS-LINK"

    # James-side relay
    RELAY_HEALTH=$(curl -s --max-time 3 "http://localhost:8766/relay/health" 2>/dev/null || echo "")
    if [ -n "$RELAY_HEALTH" ]; then
        RELAY_MODE=$(echo "$RELAY_HEALTH" | sed -n 's/.*"connection_mode":"\([^"]*\)".*/\1/p' | head -1)
        [ -z "$RELAY_MODE" ] && RELAY_MODE="?"
        ok "James relay :8766 UP (mode: ${RELAY_MODE})"
    else
        issue "James relay :8766 DOWN — comms-link not running"
    fi

    # Comms-link repo sync
    CL_DIR="${RP_DIR}/comms-link"
    if [ -d "${CL_DIR}/.git" ]; then
        CL_UNPUSHED=$(git -C "$CL_DIR" rev-list --count '@{upstream}..HEAD' 2>/dev/null || echo "?")
        CL_HEAD=$(git -C "$CL_DIR" rev-parse --short HEAD 2>/dev/null || echo "?")
        if [ "$CL_UNPUSHED" != "0" ] && [ "$CL_UNPUSHED" != "?" ]; then
            warn "comms-link has ${CL_UNPUSHED} unpushed commits (HEAD: ${CL_HEAD})"
        else
            info "comms-link synced (HEAD: ${CL_HEAD})"
        fi
    fi
fi

# ═══════════════════════════════════════════════════════════════════════
# SECTION 6: ACTION SUMMARY
# ═══════════════════════════════════════════════════════════════════════

section "SUMMARY"

if [ "$TOTAL_ISSUES" -eq 0 ] && [ "$TOTAL_WARNINGS" -eq 0 ]; then
    echo -e "  ${GREEN}${BOLD}ALL CLEAR${NC} — everything is in sync"
else
    echo -e "  ${RED}${TOTAL_ISSUES} issues${NC} | ${YELLOW}${TOTAL_WARNINGS} warnings${NC}"
    echo ""

    if [ "$TOTAL_ISSUES" -gt 0 ] || [ "$TOTAL_WARNINGS" -gt 0 ]; then
        echo -e "  ${BOLD}Recommended actions:${NC}"

        # Check for unpushed commits
        for REPO_NAME in racecontrol comms-link deploy-staging whatsapp-bot racingpoint-admin; do
            REPO_PATH="${RP_DIR}/${REPO_NAME}"
            if [ -d "${REPO_PATH}/.git" ]; then
                UP=$(git -C "$REPO_PATH" rev-list --count '@{upstream}..HEAD' 2>/dev/null || echo "0")
                if [ "$UP" -gt 0 ] 2>/dev/null; then
                    echo -e "    ${CYAN}1.${NC} cd ${REPO_NAME} && git push  ${DIM}(${UP} commits)${NC}"
                fi
            fi
        done

        # Check for stale build
        if [ -f "$MANIFEST" ]; then
            MH=$(grep 'git_hash' "$MANIFEST" 2>/dev/null | head -1 | sed 's/.*= *"//;s/".*//' || echo "")
            [ -z "$MH" ] && MH=$(grep 'git_commit' "$MANIFEST" 2>/dev/null | head -1 | sed 's/.*= *"//;s/".*//' || echo "")
            RH=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "")
            CC=$(git -C "$REPO_DIR" log --oneline "${MH}..${RH}" -- 'crates/' 2>/dev/null | wc -l || echo "0")
            if [ "$CC" -gt 0 ] 2>/dev/null; then
                echo -e "    ${CYAN}2.${NC} bash scripts/stage-release.sh  ${DIM}(${CC} code commits since last build)${NC}"
            fi
        fi

        # Check for behind targets
        if [ -n "${SERVER_BUILD:-}" ] && [ -n "${EXPECTED:-}" ] && [ "$SERVER_BUILD" != "$EXPECTED" ]; then
            echo -e "    ${CYAN}3.${NC} bash scripts/deploy-server.sh  ${DIM}(server on ${SERVER_BUILD}, staged ${EXPECTED})${NC}"
        fi

        # Pod deploy suggestion
        PODS_BEHIND=0
        for POD_NUM in $(seq 1 8); do
            POD_BUILD=$(echo "${FLEET_DATA:-[]}" | sed -n "s/.*\"pod_number\":${POD_NUM}[^}]*\"build_id\":\"\([^\"]*\)\".*/\1/p" | head -1)
            if [ -n "$POD_BUILD" ] && [ -n "${EXPECTED:-}" ] && [ "$POD_BUILD" != "$EXPECTED" ]; then
                PODS_BEHIND=$((PODS_BEHIND+1))
            fi
        done
        if [ "$PODS_BEHIND" -gt 0 ]; then
            echo -e "    ${CYAN}4.${NC} bash scripts/deploy-pod.sh all  ${DIM}(${PODS_BEHIND} pods behind)${NC}"
        fi

        # Cloud deploy suggestion
        if [ -n "${BONO_BUILD:-}" ] && [ -n "${EXPECTED:-}" ] && [ "$BONO_BUILD" != "$EXPECTED" ]; then
            echo -e "    ${CYAN}5.${NC} bash scripts/deploy-cloud.sh  ${DIM}(cloud on ${BONO_BUILD}, staged ${EXPECTED})${NC}"
        fi
    fi
fi

echo ""
echo "=========================================="
echo "  ${TOTAL_ISSUES} issues | ${TOTAL_WARNINGS} warnings"
echo "=========================================="
exit $TOTAL_ISSUES
