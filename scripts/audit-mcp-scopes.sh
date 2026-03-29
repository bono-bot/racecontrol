#!/usr/bin/env bash
# audit-mcp-scopes.sh — M3-SEC: Audit Google OAuth scopes for MCP services
#
# Checks which Google API scopes are granted to the OAuth refresh token
# used by the MCP services (calendar, drive, gmail, sheets).
#
# Run: bash scripts/audit-mcp-scopes.sh
#
# Requires: GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, GOOGLE_REFRESH_TOKEN in env

set -e

if [ -z "$GOOGLE_CLIENT_ID" ] || [ -z "$GOOGLE_CLIENT_SECRET" ] || [ -z "$GOOGLE_REFRESH_TOKEN" ]; then
  echo "ERROR: Set GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, GOOGLE_REFRESH_TOKEN"
  exit 1
fi

echo "============================================================"
echo "MCP Google OAuth Scope Audit"
echo "============================================================"
echo ""

# Exchange refresh token for access token to inspect scopes
RESPONSE=$(curl -s -X POST "https://oauth2.googleapis.com/token" \
  -d "client_id=${GOOGLE_CLIENT_ID}" \
  -d "client_secret=${GOOGLE_CLIENT_SECRET}" \
  -d "refresh_token=${GOOGLE_REFRESH_TOKEN}" \
  -d "grant_type=refresh_token")

ACCESS_TOKEN=$(echo "$RESPONSE" | python3 -c "import json,sys; print(json.load(sys.stdin).get('access_token',''))" 2>/dev/null)
SCOPE=$(echo "$RESPONSE" | python3 -c "import json,sys; print(json.load(sys.stdin).get('scope','NONE'))" 2>/dev/null)

if [ -z "$ACCESS_TOKEN" ]; then
  echo "ERROR: Could not exchange refresh token. Response:"
  echo "$RESPONSE" | head -5
  exit 1
fi

echo "Granted scopes:"
echo "$SCOPE" | tr ' ' '\n' | sort | while read -r s; do
  if [ -n "$s" ]; then
    echo "  - $s"
  fi
done

echo ""
echo "--- MINIMUM REQUIRED SCOPES ---"
echo "  Calendar: https://www.googleapis.com/auth/calendar.events"
echo "  Drive:    https://www.googleapis.com/auth/drive.readonly"
echo "  Gmail:    https://www.googleapis.com/auth/gmail.modify"
echo "  Sheets:   https://www.googleapis.com/auth/spreadsheets"
echo ""

# Flag overly broad scopes
echo "--- DANGEROUS SCOPES (should be narrowed) ---"
for danger in "https://mail.google.com" "https://www.googleapis.com/auth/drive" "https://www.googleapis.com/auth/calendar"; do
  if echo "$SCOPE" | grep -q "$danger"; then
    echo "  WARNING: $danger (too broad — use .readonly or .events variant)"
  fi
done

echo ""
echo "============================================================"
echo "To narrow scopes: revoke token in Google Cloud Console and"
echo "re-authorize with minimum required scopes only."
echo "============================================================"
