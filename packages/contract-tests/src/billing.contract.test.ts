import { describe, test, expect } from 'vitest';
import type { BillingSession, BillingSessionStatus } from '@racingpoint/types';
import billingFixture from './fixtures/billing-active.json';

const VALID_BILLING_STATUSES: BillingSessionStatus[] = [
  'pending',
  'active',
  'paused_idle',
  'paused_manual',
  'completed',
  'ended_early',
  'cancelled',
  'expired',
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
