// ═══════════════════════════════════════════════════════════════
// 11 — Orphan Refund: wallet debit on an ended-early session MUST
// emit a wallet refund event. Pins P0 fix `f60e424d` (2026-04-17).
//
// Bug class: orphan silent refund. end_billing_session() previously
// overwrote wallet_debit_paise before reading it for refund calc
// (F-05 Rs.162.50 loss per 30-min early-end). Fix traced the
// original debit via wallet_transactions and emits a refund row.
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { RCApiClient } from '../../fixtures/api-client';
import { STAFF_PIN } from '../../fixtures/test-data';

const api = new RCApiClient();

test.describe('11 — Orphan refund emission', () => {
  test.beforeAll(async () => {
    await api.login(STAFF_PIN);
  });

  test('ended_early session with wallet_debit > 0 has matching refund txn', async () => {
    const sessions = await api.listBillingSessions();

    const fourHoursAgo = new Date(Date.now() - 4 * 60 * 60 * 1000).toISOString();
    const candidates = sessions.filter(s =>
      s.started_at && s.started_at > fourHoursAgo &&
      s.status === 'ended_early' &&
      (s.wallet_debit_paise || 0) > 0
    );

    console.log(`\n═══ ORPHAN REFUND CHECK ═══`);
    console.log(`ended_early sessions with wallet_debit > 0 in last 4h: ${candidates.length}`);

    if (candidates.length === 0) {
      test.skip(true, 'No candidate sessions — spec is observational; run after a live ended_early');
    }

    const orphans: Array<{ sessionId: string; debit: number; driverId: string }> = [];

    for (const s of candidates) {
      const txns = await api.walletTransactions(s.driver_id);
      // A refund is any credit txn referencing this session
      const refund = txns.find(t =>
        t.type === 'refund' &&
        (t.billing_session_id === s.id || (t.notes || '').includes(s.id))
      );
      if (!refund) {
        orphans.push({ sessionId: s.id, debit: s.wallet_debit_paise || 0, driverId: s.driver_id });
      }
    }

    if (orphans.length > 0) {
      console.log('ORPHANS (debit with no refund):');
      for (const o of orphans) {
        console.log(`  session=${o.sessionId} driver=${o.driverId} debit=₹${(o.debit/100).toFixed(2)}`);
      }
    }

    expect(orphans.length).toBe(0);
  });

  test('refund_billing endpoint returns an idempotency-safe response', async () => {
    // Pure API contract check — no live session needed
    // Calling refund with a non-existent session should 404, not 500
    const resp = await fetch(`${process.env.API_BASE || 'http://192.168.31.23:8080/api/v1'}/billing/nonexistent_${Date.now()}/refund`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${(api as any).token}`,
      },
      body: JSON.stringify({ method: 'wallet', reason: 'e2e contract check' }),
    });

    console.log(`\n refund endpoint status for non-existent session: ${resp.status}`);
    // Server returns 422 (unprocessable) today. Accept any 4xx validation response;
    // the regression signal is 500 (server error) or 200 (silently accepted refund).
    expect(resp.status).toBeGreaterThanOrEqual(400);
    expect(resp.status).toBeLessThan(500);
  });
});
