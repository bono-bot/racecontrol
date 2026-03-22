import { describe, test, expect } from 'vitest';
import type { Pod, PodStatus, SimType } from '@racingpoint/types';
import podsFixture from './fixtures/pods.json';

const VALID_POD_STATUSES: PodStatus[] = ['offline', 'idle', 'in_session', 'error', 'disabled'];

const VALID_SIM_TYPES: SimType[] = [
  'assetto_corsa',
  'assetto_corsa_evo',
  'assetto_corsa_rally',
  'iracing',
  'le_mans_ultimate',
  'f1_25',
  'forza',
  'forza_horizon_5',
];

// Type assertion function — catches drift when Pod required fields change.
function assertPod(data: unknown): asserts data is Pod {
  const d = data as Record<string, unknown>;

  // Required fields
  expect(typeof d.id, 'id must be string').toBe('string');
  expect(typeof d.number, 'number must be number').toBe('number');
  expect(typeof d.name, 'name must be string').toBe('string');
  expect(typeof d.ip_address, 'ip_address must be string').toBe('string');

  // sim_type must be a valid SimType
  expect(
    VALID_SIM_TYPES.includes(d.sim_type as SimType),
    `sim_type "${String(d.sim_type)}" must be a valid SimType`,
  ).toBe(true);

  // status must be a valid PodStatus
  expect(
    VALID_POD_STATUSES.includes(d.status as PodStatus),
    `status "${String(d.status)}" must be a valid PodStatus`,
  ).toBe(true);
}

describe('GET /api/v1/pods — Pod contract', () => {
  test('fixture is a non-empty array', () => {
    expect(Array.isArray(podsFixture)).toBe(true);
    expect(podsFixture.length).toBeGreaterThan(0);
  });

  test('each pod matches Pod contract', () => {
    podsFixture.forEach((pod, i) => {
      try {
        assertPod(pod);
      } catch (e) {
        throw new Error(`Pod at index ${i} failed contract: ${String(e)}`);
      }
    });
  });

  test('sim_type field is a valid SimType', () => {
    podsFixture.forEach((pod) => {
      const st = (pod as Record<string, unknown>).sim_type as string;
      expect(VALID_SIM_TYPES).toContain(st);
    });
  });

  test('status field is a valid PodStatus', () => {
    podsFixture.forEach((pod) => {
      const s = (pod as Record<string, unknown>).status as string;
      expect(VALID_POD_STATUSES).toContain(s);
    });
  });
});
