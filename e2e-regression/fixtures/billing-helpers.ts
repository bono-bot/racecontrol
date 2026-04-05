// ═══════════════════════════════════════════════════════════════
// Billing Helpers — verify billing math, state transitions
// ═══════════════════════════════════════════════════════════════

import { expect } from '@playwright/test';
import { RCApiClient, BillingSession } from './api-client';
import { EndType, PER_MINUTE_RATES } from './test-data';

// Verify wallet debit matches expected tier price
export function verifyWalletDebit(session: BillingSession, expectedPricePaise: number): void {
  expect(session.wallet_debit_paise).toBe(expectedPricePaise);
}

// Compute expected refund for early-ended session
export function computeExpectedRefund(
  allocatedSeconds: number,
  drivingSeconds: number,
  walletDebitPaise: number,
): number {
  if (allocatedSeconds === 0) return 0;
  const remaining = allocatedSeconds - drivingSeconds;
  if (remaining <= 0) return 0;
  return Math.floor((remaining * walletDebitPaise) / allocatedSeconds);
}

// Compute per-minute cost (tiered, non-retroactive)
export function computePerMinuteCost(drivingSeconds: number): number {
  let totalPaise = 0;
  let remainingSeconds = drivingSeconds;

  for (const tier of PER_MINUTE_RATES) {
    const tierStart = tier.rangeStart * 60;
    const tierEnd = tier.rangeEnd === Infinity ? Infinity : tier.rangeEnd * 60;
    const tierDuration = Math.min(remainingSeconds, tierEnd - tierStart);

    if (tierDuration <= 0) break;

    // Banker's rounding: (seconds * rate_per_min + 30) / 60
    totalPaise += Math.floor((tierDuration * tier.ratePerMinPaise + 30) / 60);
    remainingSeconds -= tierDuration;

    if (remainingSeconds <= 0) break;
  }

  return totalPaise;
}

// Verify session ended with correct status
export function verifyTerminalStatus(session: BillingSession, expectedEndType: EndType): void {
  const statusMap: Record<EndType, string> = {
    completed: 'completed',
    ended_early: 'ended_early',
    cancelled: 'cancelled',
    cancelled_no_playable: 'cancelled_no_playable',
  };
  expect(session.status).toBe(statusMap[expectedEndType]);
}

// Verify wallet balance after session (debit - refund = net charge)
export async function verifyWalletAfterSession(
  api: RCApiClient,
  driverId: string,
  balanceBefore: number,
  session: BillingSession,
  endType: EndType,
): Promise<void> {
  const wallet = await api.getWallet(driverId);

  if (endType === 'cancelled' || endType === 'cancelled_no_playable') {
    // Full refund — balance should be back to before
    expect(wallet.balance_paise).toBe(balanceBefore);
  } else if (endType === 'ended_early') {
    // Proportional refund
    const expectedRefund = computeExpectedRefund(
      session.allocated_seconds,
      session.driving_seconds,
      session.wallet_debit_paise,
    );
    const expectedBalance = balanceBefore - session.wallet_debit_paise + expectedRefund;
    // Allow 1 paise tolerance for rounding
    expect(Math.abs(wallet.balance_paise - expectedBalance)).toBeLessThanOrEqual(1);
  } else if (endType === 'completed') {
    // No refund
    const expectedBalance = balanceBefore - session.wallet_debit_paise;
    expect(wallet.balance_paise).toBe(expectedBalance);
  }
}

// Verify billing events trace the correct state machine path
export async function verifyBillingEvents(
  api: RCApiClient,
  sessionId: string,
  endType: EndType,
): Promise<void> {
  const events = await api.billingSessionEvents(sessionId);

  // All sessions should have a 'started' event
  const eventTypes = events.map(e => e.event_type);

  expect(eventTypes[0]).toBe('started');

  if (endType === 'cancelled_no_playable') {
    expect(eventTypes).toContain('cancelled');
  } else if (endType === 'cancelled') {
    expect(eventTypes).toContain('cancelled');
  } else if (endType === 'ended_early') {
    expect(eventTypes).toContain('ended_early');
  } else if (endType === 'completed') {
    expect(eventTypes).toContain('ended');
  }
}

// Wait for a billing session to reach a specific status
export async function waitForStatus(
  api: RCApiClient,
  sessionId: string,
  statuses: string[],
  timeoutMs = 120_000,
): Promise<BillingSession> {
  return api.waitForBillingStatus(sessionId, statuses, timeoutMs);
}
