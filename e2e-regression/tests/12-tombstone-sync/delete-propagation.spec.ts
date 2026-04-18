// ═══════════════════════════════════════════════════════════════
// 12 — Tombstone Sync: deleting a billing_session on server must
// NOT be re-propagated from cloud within one sync cycle. Pins the
// C1 FK cleanup incident (2026-04-17): Bono VPS re-seeded 29 orphan
// sessions 13 minutes after server-side DELETE because cloud sync
// has no delete-tombstone propagation.
//
// This spec is OBSERVATIONAL today — it reads counts before/after
// a sync window. A proper active delete + re-check can be added
// once the tombstone protocol (v52.0 Phase 39x) ships.
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { RCApiClient } from '../../fixtures/api-client';
import { STAFF_PIN } from '../../fixtures/test-data';

const api = new RCApiClient();

test.describe('12 — Tombstone sync (delete propagation)', () => {
  test.beforeAll(async () => {
    await api.login(STAFF_PIN);
  });

  test('server billing_session count is stable across a sync cycle', async () => {
    // Take two snapshots ~35s apart (cloud sync interval is 30s)
    const before = await api.listBillingSessions();
    const beforeCount = before.length;
    const beforeIds = new Set(before.map(s => s.id));

    console.log(`\n═══ TOMBSTONE STABILITY CHECK ═══`);
    console.log(`Snapshot 1: ${beforeCount} sessions`);

    await new Promise(r => setTimeout(r, 35_000));

    const after = await api.listBillingSessions();
    const afterCount = after.length;
    const afterIds = new Set(after.map(s => s.id));

    console.log(`Snapshot 2: ${afterCount} sessions`);

    const reappeared: string[] = [];
    const disappeared: string[] = [];
    for (const id of beforeIds) if (!afterIds.has(id)) disappeared.push(id);
    for (const id of afterIds) if (!beforeIds.has(id)) reappeared.push(id);

    console.log(`New in snapshot 2 (legit new sessions OR sync re-propagation): ${reappeared.length}`);
    console.log(`Dropped in snapshot 2 (expected 0 — sessions do not self-delete): ${disappeared.length}`);

    // The strict claim: no session disappears without a staff delete.
    // If this fails, something is silently pruning rows (suspicious).
    expect(disappeared.length).toBe(0);
  });

  test('no orphan billing_sessions reference deleted drivers', async () => {
    // C1 FK integrity check: every billing_session.driver_id MUST
    // point to a live drivers row. Orphans were the symptom of the
    // cloud re-propagation bug.
    const sessions = await api.listBillingSessions();
    const rawDrivers: any = await api.listDrivers();
    // API may return an array or { drivers: [...] } — handle both
    const drivers: any[] = Array.isArray(rawDrivers) ? rawDrivers : (rawDrivers.drivers || []);
    const driverIds = new Set(drivers.map((d: any) => d.id));

    const orphans = sessions.filter(s => !driverIds.has(s.driver_id));

    console.log(`\nbilling_session orphans (driver_id not in drivers): ${orphans.length}`);
    if (orphans.length > 0) {
      for (const s of orphans.slice(0, 5)) {
        console.log(`  session=${s.id} driver=${s.driver_id} status=${s.status}`);
      }
    }

    // Ground truth as of 2026-04-17: 20 orphans on Bono VPS, 0 on server.
    // This assertion pins "server must stay at 0."
    expect(orphans.length).toBe(0);
  });
});
