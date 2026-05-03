#!/usr/bin/env node
// graphify-multi-root-auto-update.js
// SessionStart hook: when a tracked repo's HEAD commit is newer than its
// graphify-out/graph.json, spawn `graphify update <root>` detached so session
// startup doesn't wait.
//
// Pairs with graphify-memory-auto-update.js (memory dir uses flat-file mtime;
// this hook uses git HEAD timestamp — stable signal, ignores uncommitted churn).
// Coverage: racecontrol, comms-link, racingpoint-admin, whatsapp-bot.
// Memory has its own hook (different mtime model).
//
// Per-root lock at ~/.claude/state/graphify-bg-<root-id>.lock with same
// FRESH/STALE windows as memory hook. Sequential per root, fail-open per root.
//
// Test override: set GRAPHIFY_MULTI_ROOTS_OVERRIDE to a JSON array of
// {id, path} objects to substitute the default ROOTS table (used by .test.sh).

const fs = require("fs");
const path = require("path");
const { spawn, execFileSync } = require("child_process");

const HOME = process.env.USERPROFILE || process.env.HOME || "C:/Users/bono";
const STATE_DIR = path.join(HOME, ".claude/state");
const LOG_FILE = path.join(STATE_DIR, "graphify-bg.log");

const LOCK_STALE_MS = 30 * 60 * 1000;
const LOCK_FRESH_MS = 2 * 60 * 1000;

// Pilot layout auto-detect (Q3 Option A absorption from bono AMPLIFIER msg=34809;
// MMA D5 unanimous 3/3 vendor-disjoint convergence on cross-machine path resolution).
// james-side has racingpoint/<repo> subdir; bono-side has repos directly under HOME
// + 4th repo is racingpoint-api-gateway (not whatsapp-bot — bono has no whatsapp-bot).
// GRAPHIFY_MULTI_ROOTS_OVERRIDE env-var (below) still wins if set.
const _racingPointSubdir = path.join(HOME, "racingpoint");
const _hasRacingPointSubdir = (() => {
  try { return fs.statSync(_racingPointSubdir).isDirectory(); }
  catch (_) { return false; }
})();

const DEFAULT_ROOTS_JAMES = [
  { id: "racecontrol",       path: path.join(HOME, "racingpoint/racecontrol") },
  { id: "comms-link",        path: path.join(HOME, "racingpoint/comms-link") },
  { id: "racingpoint-admin", path: path.join(HOME, "racingpoint/racingpoint-admin") },
  { id: "whatsapp-bot",      path: path.join(HOME, "racingpoint/whatsapp-bot") },
];

const DEFAULT_ROOTS_BONO = [
  { id: "racecontrol",             path: path.join(HOME, "racecontrol") },
  { id: "comms-link",              path: path.join(HOME, "comms-link") },
  { id: "racingpoint-admin",       path: path.join(HOME, "racingpoint-admin") },
  { id: "racingpoint-api-gateway", path: path.join(HOME, "racingpoint-api-gateway") },
];

const DEFAULT_ROOTS = _hasRacingPointSubdir ? DEFAULT_ROOTS_JAMES : DEFAULT_ROOTS_BONO;

let ROOTS = DEFAULT_ROOTS;
if (process.env.GRAPHIFY_MULTI_ROOTS_OVERRIDE) {
  try { ROOTS = JSON.parse(process.env.GRAPHIFY_MULTI_ROOTS_OVERRIDE); }
  catch (_) { /* fall back to defaults */ }
}

function out(msg) { process.stdout.write("[graphify-multi-auto] " + msg + "\n"); }
function err(msg) { process.stderr.write("[graphify-multi-auto] " + msg + "\n"); }

function pidAlive(pid) {
  if (!pid) return false;
  try { process.kill(pid, 0); return true; } catch (_) { return false; }
}

function processRoot(root) {
  const graphFile = path.join(root.path, "graphify-out", "graph.json");

  if (!fs.existsSync(root.path)) {
    out(root.id + ": skipped (root not found)");
    return;
  }
  if (!fs.existsSync(graphFile)) {
    out(root.id + ": skipped (no graph.json — run /graphify " + root.path.replace(/\\/g, "/") + " to build initial graph)");
    return;
  }

  const lockFile = path.join(STATE_DIR, "graphify-bg-" + root.id + ".lock");
  if (fs.existsSync(lockFile)) {
    try {
      const raw = JSON.parse(fs.readFileSync(lockFile, "utf8"));
      const age = Date.now() - (raw.startedAt || 0);
      if (age < LOCK_FRESH_MS) {
        out(root.id + ": skipped (fired " + Math.round(age / 1000) + "s ago)");
        return;
      }
      if (raw.pid && pidAlive(raw.pid) && age < LOCK_STALE_MS) {
        out(root.id + ": already running pid=" + raw.pid + " age=" + Math.round(age / 60000) + "min");
        return;
      }
      try { fs.unlinkSync(lockFile); } catch (_) {}
    } catch (_) {
      try { fs.unlinkSync(lockFile); } catch (_) {}
    }
  }

  // Compare HEAD-commit timestamp vs graph.json mtime — stable signal,
  // unaffected by uncommitted working-tree churn.
  let lastCommitMs = 0;
  try {
    const ct = execFileSync("git", ["-C", root.path, "log", "-1", "--format=%ct", "HEAD"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
    lastCommitMs = parseInt(ct, 10) * 1000;
  } catch (e) {
    out(root.id + ": git head check failed — " + e.message.split("\n")[0]);
    return;
  }
  const graphMtime = fs.statSync(graphFile).mtimeMs;
  if (lastCommitMs <= graphMtime) {
    out(root.id + ": fresh (graph.json ≥ HEAD)");
    return;
  }
  const ageMin = Math.round((lastCommitMs - graphMtime) / 60000);

  // Test mode: skip actual spawn (env override + DRY_RUN flag combined to keep
  // .test.sh deterministic without invoking graphify CLI).
  if (process.env.GRAPHIFY_MULTI_DRY_RUN === "1") {
    fs.writeFileSync(lockFile, JSON.stringify({
      pid: 0, startedAt: Date.now(),
      args: ["update", root.path], log: LOG_FILE, dryRun: true,
    }));
    out(root.id + ": DRY_RUN would spawn graphify update (graph +" + ageMin + " min behind HEAD)");
    return;
  }

  let child;
  try {
    child = spawn("graphify", ["update", root.path], {
      detached: true,
      stdio: ["ignore", fs.openSync(LOG_FILE, "a"), fs.openSync(LOG_FILE, "a")],
      windowsHide: true,
      shell: process.platform === "win32",
    });
    child.unref();
  } catch (e) {
    err(root.id + ": spawn failed — " + e.message);
    return;
  }

  fs.writeFileSync(lockFile, JSON.stringify({
    pid: child.pid,
    startedAt: Date.now(),
    args: ["update", root.path],
    log: LOG_FILE,
  }));

  out(root.id + ": spawned graphify update pid=" + child.pid + " (graph +" + ageMin + " min behind HEAD)");
}

try {
  fs.mkdirSync(STATE_DIR, { recursive: true });
  for (const root of ROOTS) {
    try { processRoot(root); }
    catch (e) { err(root.id + " error: " + e.message); }
  }
  process.exit(0);
} catch (e) {
  err("failed: " + e.message);
  process.exit(0);
}
