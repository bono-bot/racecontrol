/**
 * Phase 368 D-09 Layout Regression Guard (P1-03)
 *
 * Asserts that the incidents sidebar + pod grid region in debug/page.tsx
 * has NOT been modified. The SHA256 of the marker-bracketed region is
 * committed as a fixture and must match on every test run.
 *
 * If this test fails after a Plan 368 edit, it means the executor touched
 * the D-09 locked region. Revert those lines — they are off-limits.
 */
import { describe, it, expect } from "vitest";
import * as fs from "fs";
import * as crypto from "crypto";
import * as path from "path";

describe("Phase 368 D-09 — debug page incidents region untouched (P1-03)", () => {
  it("incidents sidebar + pod grid SHA matches the committed fixture", () => {
    const pageSrc = fs.readFileSync(
      path.join(__dirname, "../page.tsx"),
      "utf8"
    );

    const startMarker = "{/* ── DEBUG-PAGE-INCIDENTS-REGION-START ── */}";
    const endMarker   = "{/* ── DEBUG-PAGE-INCIDENTS-REGION-END ── */}";

    const start = pageSrc.indexOf(startMarker);
    const end   = pageSrc.indexOf(endMarker);

    expect(start, "START marker not found in debug/page.tsx").toBeGreaterThanOrEqual(0);
    expect(end, "END marker not found in debug/page.tsx").toBeGreaterThan(start);

    const region = pageSrc.slice(start, end + endMarker.length);
    const sha = crypto.createHash("sha256").update(region).digest("hex");

    // Fixture is at racecontrol/tests/fixtures/368-debug-page-incidents-sha.txt
    // Test lives at kiosk/src/app/debug/__tests__/ — 5 dirs up to repo root
    const fixturePath = path.resolve(
      __dirname,
      "../../../../../tests/fixtures/368-debug-page-incidents-sha.txt"
    );

    const expected = fs.readFileSync(fixturePath, "utf8").trim();
    expect(sha, `SHA mismatch — incidents region was modified.\nComputed: ${sha}\nExpected: ${expected}`).toBe(expected);
  });
});
