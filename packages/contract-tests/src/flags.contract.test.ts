import { describe, test, expect } from 'vitest';
import type { FeatureFlag } from '@racingpoint/types';
import flagsFixture from './fixtures/flags.json';

function assertFeatureFlag(data: unknown): asserts data is FeatureFlag {
  const d = data as Record<string, unknown>;
  expect(typeof d.name, 'name must be string').toBe('string');
  expect(typeof d.enabled, 'enabled must be boolean').toBe('boolean');
  expect(typeof d.default_value, 'default_value must be boolean').toBe('boolean');
  expect(typeof d.version, 'version must be number').toBe('number');
  expect(d.overrides !== null && typeof d.overrides === 'object', 'overrides must be object').toBe(true);
  expect(typeof d.updated_at, 'updated_at must be string').toBe('string');
}

describe('GET /api/v1/flags - FeatureFlag contract', () => {
  test('fixture is a non-empty array', () => {
    expect(Array.isArray(flagsFixture)).toBe(true);
    expect(flagsFixture.length).toBeGreaterThan(0);
  });

  test('each flag matches FeatureFlag contract', () => {
    flagsFixture.forEach((flag, i) => {
      try { assertFeatureFlag(flag); }
      catch (e) { throw new Error(`Flag at index ${i} failed: ${String(e)}`); }
    });
  });

  test('overrides values are booleans when present', () => {
    flagsFixture.forEach((flag) => {
      const overrides = (flag as Record<string, unknown>).overrides as Record<string, unknown>;
      Object.values(overrides).forEach(v => {
        expect(typeof v, 'override value must be boolean').toBe('boolean');
      });
    });
  });
});
