import { describe, test, expect } from 'vitest';
import type { BillingSession, BillingSessionStatus } from '@racingpoint/types';
import billingFixture from './fixtures/billing-active.json';

const VALID_BILLING_STATUSES: BillingSessionStatus[] = [
  'pending',
  'waiting_for_game',
  'active',
  'paused_manual',
  'paused_disconnect',
  'paused_game_pause',
  'completed',
  'ended_early',
  'cancelled',
  'cancelled_no_playable',
];

const VALID_DRIVING_STATES = ['active', 'idle', 'no_device'] as const;

// Type assertion function — catches drift when BillingSession required fields change.
function assertBillingSession(data: unknown): asserts data is BillingSession {
  const d = data as Record<string, unknown>;

  // Required string fields
  expect(typeof d.id, 'id must be string').toBe('string');
  expect(typeof d.driver_id, 'driver_id must be string').toBe('string');
  expect(typeof d.driver_name, 'driver_name must be string').toBe('string');
  expect(typeof d.pod_id, 'pod_id must be string').toBe('string');
  expect(typeof d.pricing_tier_name, 'pricing_tier_name must be string').toBe('string');

  // Required number fields
  expect(typeof d.allocated_seconds, 'allocated_seconds must be number').toBe('number');
  expect(typeof d.driving_seconds, 'driving_seconds must be number').toBe('number');
  expect(typeof d.remaining_seconds, 'remaining_seconds must be number').toBe('number');
  expect(typeof d.split_count, 'split_count must be number').toBe('number');
  expect(typeof d.current_split_number, 'current_split_number must be number').toBe('number');

  // Status must be a valid BillingSessionStatus value
  expect(
    VALID_BILLING_STATUSES.includes(d.status as BillingSessionStatus),
    `status "${String(d.status)}" must be a valid BillingSessionStatus`,
  ).toBe(true);

  // driving_state must be a valid DrivingState value
  expect(
    VALID_DRIVING_STATES.includes(d.driving_state as (typeof VALID_DRIVING_STATES)[number]),
    `driving_state "${String(d.driving_state)}" must be one of ${VALID_DRIVING_STATES.join(', ')}`,
  ).toBe(true);
}

describe('BillingSessionStatus — variant completeness', () => {
  test('BillingSessionStatus has exactly 10 variants', () => {
    expect(VALID_BILLING_STATUSES.length).toBe(10);
  });

  test('includes waiting_for_game variant', () => {
    expect(VALID_BILLING_STATUSES).toContain('waiting_for_game');
  });

  test('includes paused_disconnect variant', () => {
    expect(VALID_BILLING_STATUSES).toContain('paused_disconnect');
  });

  test('includes paused_game_pause variant', () => {
    expect(VALID_BILLING_STATUSES).toContain('paused_game_pause');
  });

  test('includes cancelled_no_playable variant', () => {
    expect(VALID_BILLING_STATUSES).toContain('cancelled_no_playable');
  });
});

describe('GET /api/v1/billing/sessions — BillingSession contract', () => {
  test('fixture is a non-empty array', () => {
    expect(Array.isArray(billingFixture)).toBe(true);
    expect(billingFixture.length).toBeGreaterThan(0);
  });

  test('each session matches BillingSession contract', () => {
    billingFixture.forEach((session, i) => {
      try {
        assertBillingSession(session);
      } catch (e) {
        throw new Error(`Session at index ${i} failed contract: ${String(e)}`);
      }
    });
  });

  test('status field is a valid BillingSessionStatus', () => {
    billingFixture.forEach((session) => {
      const s = (session as Record<string, unknown>).status as string;
      expect(VALID_BILLING_STATUSES).toContain(s);
    });
  });

  test('driving_state field is a valid DrivingState', () => {
    billingFixture.forEach((session) => {
      const ds = (session as Record<string, unknown>).driving_state as string;
      expect(VALID_DRIVING_STATES as readonly string[]).toContain(ds);
    });
  });
});

// Phase 414 — between_games_idle_seconds field (414-CONTRACT-01)
// Plan 03 removed .skip — BillingSession now has between_games_idle_seconds in @racingpoint/types.
describe('Phase 414 — BillingSession.between_games_idle_seconds field', () => {
  test('field is optional number on BillingSession type', () => {
    // 414-CONTRACT-01: shape assertion. Type-only — actual value population is server-side.
    // BillingSession.between_games_idle_seconds is now part of @racingpoint/types (Plan 03).
    const sample: BillingSession = {
      id: 'sess-414',
      driver_id: 'drv-1',
      driver_name: 'Test Driver',
      pod_id: 'pod_3',
      pricing_tier_name: 'per-minute',
      allocated_seconds: 86400,
      driving_seconds: 1500,
      remaining_seconds: 84900,
      status: 'waiting_for_game',
      driving_state: 'idle',
      split_count: 1,
      current_split_number: 1,
      between_games_idle_seconds: 720,
    };
    expect(
      typeof sample.between_games_idle_seconds === 'number' ||
        sample.between_games_idle_seconds === undefined,
    ).toBe(true);
    expect(sample.between_games_idle_seconds).toBe(720);

    // Also verify that omitting the field is valid (it's optional)
    const sampleWithout: BillingSession = {
      id: 'sess-415',
      driver_id: 'drv-2',
      driver_name: 'Another Driver',
      pod_id: 'pod_1',
      pricing_tier_name: 'per-minute',
      allocated_seconds: 86400,
      driving_seconds: 900,
      remaining_seconds: 85500,
      status: 'active',
      driving_state: 'active',
      split_count: 1,
      current_split_number: 1,
    };
    expect(sampleWithout.between_games_idle_seconds).toBeUndefined();
  });
});
