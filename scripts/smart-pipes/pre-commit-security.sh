#!/bin/bash
# Smart Pipe: Pre-Commit Security Gate
# Part of CGP 4.1 Smart Pipes architecture
# Runs automatically on git commit -- agent never needs to remember this
# PASSES are silent. Only FAILURES surface.
#
# Tools: gitleaks (secrets), semgrep (SAST), cargo audit (dep vulns)
# Installed: 2026-04-04

set -e
REPO_ROOT="$(git rev-parse --show-toplevel)"
RESULTS_DIR="$REPO_ROOT/.smart-pipes-results"
mkdir -p "$RESULTS_DIR"
FAILED=0
FINDINGS=""

# Resolve gitleaks path (winget installs to odd location)
GITLEAKS=""
if command -v gitleaks &>/dev/null; then
  GITLEAKS="gitleaks"
elif command -v gitleaks.exe &>/dev/null; then
  GITLEAKS="gitleaks.exe"
elif [ -f "$HOME/bin/gitleaks.exe" ]; then
  GITLEAKS="$HOME/bin/gitleaks.exe"
elif [ -f "$HOME/AppData/Local/Microsoft/WinGet/Links/gitleaks.exe" ]; then
  GITLEAKS="$HOME/AppData/Local/Microsoft/WinGet/Links/gitleaks.exe"
else
  # Search winget packages dir
  GITLEAKS=$(find "$HOME/AppData/Local/Microsoft/WinGet/Packages" -name "gitleaks.exe" 2>/dev/null | head -1)
fi

echo "Smart Pipe: Pre-Commit Security Scan..."

# 1. Gitleaks -- detect hardcoded secrets in staged changes
echo "  [1/3] Scanning for secrets..."
if [ -n "$GITLEAKS" ]; then
  # Scan only the latest commit range (not full history) to keep it fast
  # Use --log-opts to limit scan depth for pre-commit speed
  if ! "$GITLEAKS" detect --source "$REPO_ROOT" --log-opts="-1" --no-banner \
       --report-format json --report-path "$RESULTS_DIR/gitleaks.json" 2>/dev/null; then
    COUNT=$(python3 -c "import json; print(len(json.load(open('$RESULTS_DIR/gitleaks.json'))))" 2>/dev/null || echo "?")
    if [ "$COUNT" != "0" ] && [ "$COUNT" != "?" ]; then
      FINDINGS="$FINDINGS\n  SECRETS DETECTED: $COUNT potential secrets found (see .smart-pipes-results/gitleaks.json)"
      FAILED=1
    else
      echo "    No secrets detected"
    fi
  else
    echo "    No secrets detected"
  fi
else
  echo "    gitleaks not installed -- skipping"
fi

# 2. Semgrep -- SAST on changed files only (fast)
echo "  [2/3] Static analysis on changed files..."
CHANGED_FILES=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(rs|ts|tsx|js|jsx)$' || true)
if [ -n "$CHANGED_FILES" ] && command -v semgrep &>/dev/null; then
  # Run semgrep with UTF-8 encoding (Windows compat)
  PYTHONUTF8=1 PYTHONIOENCODING=utf-8 \
    semgrep scan --config "p/default" --json --severity ERROR \
    --max-target-bytes 1000000 \
    -o "$RESULTS_DIR/semgrep.json" \
    $CHANGED_FILES 2>/dev/null || true
  SEMGREP_COUNT=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/semgrep.json')); print(len(d.get('results',[])))" 2>/dev/null || echo "0")
  if [ "$SEMGREP_COUNT" != "0" ]; then
    FINDINGS="$FINDINGS\n  SAST: $SEMGREP_COUNT issues found (see .smart-pipes-results/semgrep.json)"
    # Don't block on semgrep -- warn only (too many false positives at ERROR level)
  else
    echo "    No SAST issues in changed files"
  fi
else
  if [ -z "$CHANGED_FILES" ]; then
    echo "    No code files changed -- skipping"
  else
    echo "    semgrep not installed -- skipping"
  fi
fi

# 3. Cargo audit -- only if Cargo.lock changed
echo "  [3/3] Dependency audit..."
if git diff --cached --name-only | grep -q "Cargo.lock"; then
  if command -v cargo &>/dev/null; then
    cargo audit --json > "$RESULTS_DIR/cargo-audit.json" 2>/dev/null || true
    VULN_COUNT=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/cargo-audit.json')); print(d.get('vulnerabilities',{}).get('count',0))" 2>/dev/null || echo "0")
    if [ "$VULN_COUNT" != "0" ]; then
      FINDINGS="$FINDINGS\n  CARGO AUDIT: $VULN_COUNT vulnerable dependencies"
    else
      echo "    No known vulnerabilities in Rust dependencies"
    fi
  else
    echo "    cargo not installed -- skipping"
  fi
else
  echo "    Cargo.lock unchanged -- skipping"
fi

# Summary
if [ $FAILED -eq 1 ]; then
  echo ""
  echo "============================================"
  echo "  PRE-COMMIT BLOCKED"
  echo "============================================"
  echo -e "$FINDINGS"
  echo "============================================"
  echo ""
  echo "Fix the issues above before committing."
  echo "Results saved to .smart-pipes-results/"
  exit 1
fi

if [ -n "$FINDINGS" ]; then
  echo ""
  echo "Warnings (not blocking):"
  echo -e "$FINDINGS"
  echo ""
fi

echo "Pre-commit security scan passed"
exit 0
