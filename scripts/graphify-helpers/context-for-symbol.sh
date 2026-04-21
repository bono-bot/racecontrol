#!/usr/bin/env bash
# scripts/graphify-helpers/context-for-symbol.sh — Tier 4 workflow helper.
#
# Given a symbol name, emit JSON with every occurrence across unified graph
# + up to N neighbour labels. Intended for feeding context into MMA prompts,
# debug sessions, or LLM diagnostic workflows — alternative to grep when you
# want structural context not textual context.
#
# Usage:
#   scripts/graphify-helpers/context-for-symbol.sh <symbol> [neighbour-count]
#
# Example:
#   scripts/graphify-helpers/context-for-symbol.sh handle_ws_message 5
#
# Requires: node, scripts/graphify-post/unify.mjs output
#   at graphify-out-unified/graph.json. Run `node scripts/graphify-post/unify.mjs`
#   first if it doesn't exist.

set -euo pipefail

SYMBOL="${1:-}"
NEIGHBOURS="${2:-5}"

if [ -z "$SYMBOL" ]; then
  echo "usage: $0 <symbol> [neighbour-count]" >&2
  exit 2
fi

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
GRAPH="$REPO_ROOT/graphify-out-unified/graph.json"

if [ ! -f "$GRAPH" ]; then
  echo "error: unified graph missing at $GRAPH" >&2
  echo "       run: node scripts/graphify-post/unify.mjs" >&2
  exit 3
fi

node --input-type=module -e "
import fs from 'fs';
const g = JSON.parse(fs.readFileSync('$GRAPH', 'utf8'));
const symbol = '$SYMBOL';
const K = parseInt('$NEIGHBOURS', 10);

// Graphify labels Rust/JS functions with trailing () — accept both forms.
const variants = [symbol, symbol + '()', symbol.replace(/\(\)$/, '')];
const matches = g.nodes.filter(n => {
  const label = (n.label || n.id || '').toString();
  for (const v of variants) {
    if (label === v || label.endsWith('::' + v) || label.endsWith('.' + v)) return true;
  }
  return false;
});

const adjacencyOut = new Map();
const adjacencyIn  = new Map();
for (const e of g.links) {
  if (!adjacencyOut.has(e.source)) adjacencyOut.set(e.source, []);
  adjacencyOut.get(e.source).push({ target: e.target, relation: e.relation || 'link' });
  if (!adjacencyIn.has(e.target)) adjacencyIn.set(e.target, []);
  adjacencyIn.get(e.target).push({ source: e.source, relation: e.relation || 'link' });
}

const byId = new Map(g.nodes.map(n => [n.id, n]));

const result = matches.map(n => {
  const out = (adjacencyOut.get(n.id) || []).slice(0, K).map(e => ({
    target: byId.get(e.target)?.label || e.target,
    relation: e.relation,
  }));
  const ins = (adjacencyIn.get(n.id) || []).slice(0, K).map(e => ({
    source: byId.get(e.source)?.label || e.source,
    relation: e.relation,
  }));
  return {
    id: n.id,
    label: n.label || n.id,
    source_repo: n.source_repo || '(unknown)',
    community: n.community,
    outgoing: out,
    incoming: ins,
  };
});

console.log(JSON.stringify({
  symbol,
  matches: matches.length,
  results: result,
}, null, 2));
"
