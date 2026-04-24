// tests/fleet-probe/orchestrator-dry-run.test.mjs -- Phase 448 Plan 07
// Unit tests for probe-all.sh orchestrator: --dry-run, --help.
// No network calls -- all tests run offline.
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";

// Resolve real Python interpreter path (avoids Windows Store python3 stub that hangs).
// On Windows/Git Bash 'python3' resolves to the App Execution Alias in WindowsApps which
// hangs (pops up Microsoft Store UI) when called without an interactive session.
// The real Python 3.12 installation on Windows is accessible as 'python'.
// On Linux (production VPS), 'python3' is correct.
const PYTHON_CMD = process.platform === "win32" ? "python" : "python3";

// All 15 expected targets in locked order (per CONTEXT.md + RESEARCH section 3).
const ALL_TARGETS = [
  "server_23",
  "pod_1", "pod_2", "pod_3", "pod_4",
  "pod_5", "pod_6", "pod_7", "pod_8",
  "pos_130", "james_27", "bono_vps",
  "cloud_admin", "cloud_racecontrol", "relay_james",
];

test("probe-all.sh --dry-run exits 0 and enumerates exactly 15 targets", () => {
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-all.sh", "--dry-run"], {
    encoding: "utf8",
    timeout: 15_000,
    env: { ...process.env, PROBE_PYTHON: PYTHON_CMD },
  });
  assert.equal(
    res.status,
    0,
    `Expected exit 0, got ${res.status} stderr=${res.stderr}`
  );
  const lines = res.stdout.trim().split(/\r?\n/).filter((l) => l.length > 0);
  assert.equal(
    lines.length,
    15,
    `Expected 15 target lines, got ${lines.length}:\n${res.stdout}`
  );
  for (const t of ALL_TARGETS) {
    assert.ok(
      lines.some((l) => l.includes(`target=${t}`)),
      `Target '${t}' not found in --dry-run output:\n${res.stdout}`
    );
  }
});

test("probe-all.sh --help exits 0 and prints Usage:", () => {
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-all.sh", "--help"], {
    encoding: "utf8",
    timeout: 10_000,
    env: { ...process.env, PROBE_PYTHON: PYTHON_CMD },
  });
  assert.equal(
    res.status,
    0,
    `Expected exit 0, got ${res.status}`
  );
  assert.match(res.stdout, /Usage:/, "--help output must contain 'Usage:'");
});
