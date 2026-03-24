import { describe, test, expect } from 'vitest';
import type { ConfigPush } from '@racingpoint/types';
import configFixture from './fixtures/config-push.json';

function assertConfigPush(data: unknown): asserts data is ConfigPush {
  const d = data as Record<string, unknown>;
  expect(typeof d.id, 'id must be number').toBe('number');
  expect(typeof d.pod_id, 'pod_id must be string').toBe('string');
  expect(d.payload !== null && typeof d.payload === 'object', 'payload must be object').toBe(true);
  expect(typeof d.seq_num, 'seq_num must be number').toBe('number');
  expect(typeof d.status, 'status must be string').toBe('string');
  expect(['pending', 'delivered', 'acked'].includes(d.status as string), 'status must be pending|delivered|acked').toBe(true);
  expect(typeof d.created_at, 'created_at must be string').toBe('string');
  // acked_at is optional
  if (d.acked_at !== undefined && d.acked_at !== null) {
    expect(typeof d.acked_at, 'acked_at must be string when present').toBe('string');
  }
}

describe('GET /api/v1/config/push/queue - ConfigPush contract', () => {
  test('fixture is a non-empty array', () => {
    expect(Array.isArray(configFixture)).toBe(true);
    expect(configFixture.length).toBeGreaterThan(0);
  });

  test('each entry matches ConfigPush contract', () => {
    configFixture.forEach((entry, i) => {
      try { assertConfigPush(entry); }
      catch (e) { throw new Error(`Config push at index ${i} failed: ${String(e)}`); }
    });
  });

  test('seq_nums are unique and positive', () => {
    const seqs = configFixture.map(e => (e as Record<string, unknown>).seq_num as number);
    const unique = new Set(seqs);
    expect(unique.size).toBe(seqs.length);
    seqs.forEach(s => expect(s).toBeGreaterThan(0));
  });
});
