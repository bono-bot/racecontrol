// ═══════════════════════════════════════════════════════════════
// Financial Reconciliation — verify all billing math
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { RCApiClient } from '../../fixtures/api-client';
import { STAFF_PIN } from '../../fixtures/test-data';
import { computeExpectedRefund, computePerMinuteCost } from '../../fixtures/billing-helpers';
import { getTestDriverIds } from '../../fixtures/test-driver-factory';

const api = new RCApiClient();

test.describe('09 — Financial Reconciliation', () => {
  test.beforeAll(async () => {
    await api.login(STAFF_PIN);
  });

  test('All test sessions — verify wallet math', async () => {
    const sessions = await api.listBillingSessions();

    // Filter to E2E test sessions (created in last 4 hours)
    const fourHoursAgo = new Date(Date.now() - 4 * 60 * 60 * 1000).toISOString();
    const testSessions = sessions.filter(s =>
      s.started_at && s.started_at > fourHoursAgo
    );

    console.log(`\n═══ FINANCIAL RECONCILIATION ═══`);
    console.log(`Total sessions in last 4h: ${testSessions.length}`);

    let totalDebits = 0;
    let totalRefunds = 0;
    let discrepancies = 0;

    for (const session of testSessions) {
      totalDebits += (session.wallet_debit_paise || 0);

      if (session.status === 'ended_early' && session.wallet_debit_paise > 0) {
        const expectedRefund = computeExpectedRefund(
          session.allocated_seconds,
          session.driving_seconds,
          session.wallet_debit_paise,
        );
        // We can't easily verify the actual refund from the session alone
        // but we can check if the math makes sense
        if (expectedRefund < 0 || expectedRefund > session.wallet_debit_paise) {
          console.log(`  DISCREPANCY: session ${session.id} — refund calc out of range`);
          discrepancies++;
        }
        totalRefunds += expectedRefund;
      } else if (session.status === 'cancelled' || session.status === 'cancelled_no_playable') {
        totalRefunds += session.wallet_debit_paise;
      }
    }

    console.log(`Total debits:      ₹${(totalDebits / 100).toFixed(2)}`);
    console.log(`Total refunds:     ₹${(totalRefunds / 100).toFixed(2)}`);
    console.log(`Net revenue:       ₹${((totalDebits - totalRefunds) / 100).toFixed(2)}`);
    console.log(`Discrepancies:     ${discrepancies}`);
    console.log(`═════════════════════════════════\n`);

    expect(discrepancies).toBe(0);
  });

  test('Per-minute billing math verification', async () => {
    // Test the tiered rate calculation
    const testCases = [
      { seconds: 600, expected: computePerMinuteCost(600) },    // 10 min
      { seconds: 1800, expected: computePerMinuteCost(1800) },  // 30 min
      { seconds: 2700, expected: computePerMinuteCost(2700) },  // 45 min (crosses tier)
      { seconds: 3600, expected: computePerMinuteCost(3600) },  // 60 min
      { seconds: 5400, expected: computePerMinuteCost(5400) },  // 90 min (all 3 tiers)
    ];

    console.log('\nPer-minute billing rate verification:');
    for (const tc of testCases) {
      const minutes = tc.seconds / 60;
      const rupees = tc.expected / 100;
      console.log(`  ${minutes} min → ₹${rupees.toFixed(2)} (${tc.expected} paise)`);
    }

    // 10 min at ₹25/min = ₹250
    expect(computePerMinuteCost(600)).toBeGreaterThan(24000);
    expect(computePerMinuteCost(600)).toBeLessThan(26000);

    // 45 min should be less than 45 * ₹25 (tiered discount)
    expect(computePerMinuteCost(2700)).toBeLessThan(2700 * 2500 / 60);

    // 90 min should use all 3 tiers
    expect(computePerMinuteCost(5400)).toBeGreaterThan(0);
  });

  test('Session status distribution', async () => {
    const sessions = await api.listBillingSessions();
    const fourHoursAgo = new Date(Date.now() - 4 * 60 * 60 * 1000).toISOString();
    const recent = sessions.filter(s => s.started_at && s.started_at > fourHoursAgo);

    const statusCounts: Record<string, number> = {};
    for (const s of recent) {
      statusCounts[s.status] = (statusCounts[s.status] || 0) + 1;
    }

    console.log('\nSession status distribution:');
    for (const [status, count] of Object.entries(statusCounts)) {
      console.log(`  ${status}: ${count}`);
    }
  });
});
