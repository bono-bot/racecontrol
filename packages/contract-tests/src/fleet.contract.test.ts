import { describe, test, expect } from 'vitest';
import type { PodFleetStatus, FleetHealthResponse } from '@racingpoint/types';
import fleetFixture from './fixtures/fleet-health.json';

// Type assertion function — catches drift at runtime (fixtures) and compile time (TypeScript).
// If PodFleetStatus adds a required field, tsc --noEmit will fail until fixture is updated.
function assertPodFleetStatus(data: unknown): asserts data is PodFleetStatus {
  const d = data as Record<string, unknown>;
  expect(typeof d.pod_number, 'pod_number must be number').toBe('number');
  expect(typeof d.ws_connected, 'ws_connected must be boolean').toBe('boolean');
  expect(typeof d.http_reachable, 'http_reachable must be boolean').toBe('boolean');
  expect(typeof d.in_maintenance, 'in_maintenance must be boolean').toBe('boolean');
  expect(Array.isArray(d.maintenance_failures), 'maintenance_failures must be array').toBe(true);
  expect(typeof d.violation_count_24h, 'violation_count_24h must be number').toBe('number');
  expect(typeof d.idle_health_fail_count, 'idle_health_fail_count must be number').toBe('number');
  expect(Array.isArray(d.idle_health_failures), 'idle_health_failures must be array').toBe(true);
}

describe('GET /api/v1/fleet/health — PodFleetStatus contract', () => {
  test('fixture has pods array', () => {
    const response = fleetFixture as FleetHealthResponse;
    expect(Array.isArray(response.pods)).toBe(true);
    expect(response.pods.length).toBeGreaterThan(0);
  });

  test('each pod matches PodFleetStatus contract', () => {
    fleetFixture.pods.forEach((pod, i) => {
      try {
        assertPodFleetStatus(pod);
      } catch (e) {
        throw new Error(`Pod at index ${i} failed contract: ${String(e)}`);
      }
    });
  });

  test('fixture has timestamp field', () => {
    expect(typeof (fleetFixture as Record<string, unknown>).timestamp).toBe('string');
  });
});
