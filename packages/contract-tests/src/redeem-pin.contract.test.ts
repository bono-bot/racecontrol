import { describe, test, expect } from 'vitest';
import type { RedeemPinResponse, RedeemPinStatus } from '@racingpoint/types';
import fixtures from './fixtures/redeem-pin.json';

const VALID_STATUSES: RedeemPinStatus[] = ['lockout', 'invalid_pin', 'pending_debit', 'error'];

/** Type assertion — catches drift when RedeemPinResponse fields change. */
function assertSuccessResponse(data: unknown): asserts data is RedeemPinResponse {
  const d = data as Record<string, unknown>;

  // Required success fields
  expect(typeof d.pod_number, 'pod_number must be number').toBe('number');
  expect(d.pod_number, 'pod_number must be > 0').toBeGreaterThan(0);
  expect(typeof d.pod_id, 'pod_id must be string').toBe('string');
  expect(typeof d.driver_name, 'driver_name must be string').toBe('string');
  expect(typeof d.experience_name, 'experience_name must be string').toBe('string');
  expect(typeof d.tier_name, 'tier_name must be string').toBe('string');
  expect(typeof d.allocated_seconds, 'allocated_seconds must be number').toBe('number');
  expect(d.allocated_seconds, 'allocated_seconds must be > 0').toBeGreaterThan(0);
  expect(typeof d.billing_session_id, 'billing_session_id must be string').toBe('string');

  // Should NOT have error fields
  expect(d.error, 'success response must not have error').toBeUndefined();
  expect(d.status, 'success response must not have status').toBeUndefined();
}

function assertErrorResponse(data: unknown, expectedStatus: RedeemPinStatus): asserts data is RedeemPinResponse {
  const d = data as Record<string, unknown>;

  // Required error fields
  expect(typeof d.error, 'error must be string').toBe('string');
  expect((d.error as string).length, 'error must be non-empty').toBeGreaterThan(0);
  expect(typeof d.status, 'status must be string').toBe('string');
  expect(VALID_STATUSES).toContain(d.status);
  expect(d.status, `status must be ${expectedStatus}`).toBe(expectedStatus);

  // Should NOT have success-only fields
  expect(d.pod_number, 'error response must not have pod_number').toBeUndefined();
  expect(d.billing_session_id, 'error response must not have billing_session_id').toBeUndefined();
}

describe('POST /api/v1/kiosk/redeem-pin — RedeemPinResponse contract', () => {
  test('success fixture matches RedeemPinResponse (success shape)', () => {
    assertSuccessResponse(fixtures.success);
  });

  test('invalid_pin fixture matches RedeemPinResponse (error shape)', () => {
    assertErrorResponse(fixtures.error_invalid_pin, 'invalid_pin');
    const d = fixtures.error_invalid_pin as Record<string, unknown>;
    expect(typeof d.remaining_attempts, 'remaining_attempts must be number for invalid_pin').toBe('number');
    expect(d.remaining_attempts as number, 'remaining_attempts must be >= 0').toBeGreaterThanOrEqual(0);
  });

  test('lockout fixture matches RedeemPinResponse (lockout shape)', () => {
    assertErrorResponse(fixtures.error_lockout, 'lockout');
    const d = fixtures.error_lockout as Record<string, unknown>;
    expect(typeof d.lockout_remaining_seconds, 'lockout_remaining_seconds must be number').toBe('number');
    expect(d.lockout_remaining_seconds as number, 'lockout_remaining_seconds must be > 0').toBeGreaterThan(0);
  });

  test('pending_debit fixture matches RedeemPinResponse (pending shape)', () => {
    assertErrorResponse(fixtures.error_pending_debit, 'pending_debit');
    // Should NOT have remaining_attempts (not a PIN error)
    const d = fixtures.error_pending_debit as Record<string, unknown>;
    expect(d.remaining_attempts, 'pending_debit should not have remaining_attempts').toBeUndefined();
  });

  test('infra error fixture matches RedeemPinResponse (infra shape)', () => {
    assertErrorResponse(fixtures.error_infra, 'error');
    // Should NOT have remaining_attempts (not a PIN error)
    const d = fixtures.error_infra as Record<string, unknown>;
    expect(d.remaining_attempts, 'infra error should not have remaining_attempts').toBeUndefined();
  });

  test('allocated_seconds is a multiple of 60 (minutes precision)', () => {
    const seconds = (fixtures.success as Record<string, unknown>).allocated_seconds as number;
    expect(seconds % 60, 'allocated_seconds should be in whole minutes').toBe(0);
  });

  test('pod_number is a positive integer in valid range', () => {
    const num = (fixtures.success as Record<string, unknown>).pod_number as number;
    expect(Number.isInteger(num), 'pod_number must be integer').toBe(true);
    expect(num).toBeGreaterThanOrEqual(1);
    expect(num).toBeLessThanOrEqual(20); // Max 20 pods in venue
  });
});
