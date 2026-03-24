import { describe, test, expect } from 'vitest';
import type {
  FlagSyncPayload, WsConfigPushPayload, OtaDownloadPayload,
  KillSwitchPayload, ConfigAckPayload, OtaAckPayload, FlagCacheSyncPayload
} from '@racingpoint/types';
import fixtures from './fixtures/ws-messages.json';

function assertFlagSync(data: unknown): asserts data is FlagSyncPayload {
  const d = data as Record<string, unknown>;
  expect(d.flags !== null && typeof d.flags === 'object', 'flags must be object').toBe(true);
  expect(typeof d.version, 'version must be number').toBe('number');
  // Verify all flag values are booleans
  Object.values(d.flags as Record<string, unknown>).forEach(v => {
    expect(typeof v, 'flag value must be boolean').toBe('boolean');
  });
}

function assertConfigPush(data: unknown): asserts data is WsConfigPushPayload {
  const d = data as Record<string, unknown>;
  expect(d.fields !== null && typeof d.fields === 'object', 'fields must be object').toBe(true);
  expect(typeof d.schema_version, 'schema_version must be number').toBe('number');
  expect(typeof d.sequence, 'sequence must be number').toBe('number');
}

function assertOtaDownload(data: unknown): asserts data is OtaDownloadPayload {
  const d = data as Record<string, unknown>;
  expect(typeof d.manifest_url, 'manifest_url must be string').toBe('string');
  expect(typeof d.binary_sha256, 'binary_sha256 must be string').toBe('string');
  expect(typeof d.version, 'version must be string').toBe('string');
}

function assertKillSwitch(data: unknown): asserts data is KillSwitchPayload {
  const d = data as Record<string, unknown>;
  expect(typeof d.flag_name, 'flag_name must be string').toBe('string');
  expect(typeof d.active, 'active must be boolean').toBe('boolean');
  // reason is optional
  if (d.reason !== undefined) {
    expect(typeof d.reason, 'reason must be string when present').toBe('string');
  }
}

function assertConfigAck(data: unknown): asserts data is ConfigAckPayload {
  const d = data as Record<string, unknown>;
  expect(typeof d.pod_id, 'pod_id must be string').toBe('string');
  expect(typeof d.sequence, 'sequence must be number').toBe('number');
  expect(typeof d.accepted, 'accepted must be boolean').toBe('boolean');
}

function assertOtaAck(data: unknown): asserts data is OtaAckPayload {
  const d = data as Record<string, unknown>;
  expect(typeof d.pod_id, 'pod_id must be string').toBe('string');
  expect(typeof d.version, 'version must be string').toBe('string');
  expect(typeof d.success, 'success must be boolean').toBe('boolean');
  if (d.error !== undefined) {
    expect(typeof d.error, 'error must be string when present').toBe('string');
  }
}

function assertFlagCacheSync(data: unknown): asserts data is FlagCacheSyncPayload {
  const d = data as Record<string, unknown>;
  expect(typeof d.pod_id, 'pod_id must be string').toBe('string');
  expect(typeof d.cached_version, 'cached_version must be number').toBe('number');
}

describe('WebSocket Message Payloads - TS/Rust contract (SYNC-03)', () => {
  test('FlagSync fixture matches FlagSyncPayload', () => assertFlagSync(fixtures.flag_sync));
  test('ConfigPush fixture matches WsConfigPushPayload', () => assertConfigPush(fixtures.config_push));
  test('OtaDownload fixture matches OtaDownloadPayload', () => assertOtaDownload(fixtures.ota_download));
  test('KillSwitch fixture matches KillSwitchPayload', () => assertKillSwitch(fixtures.kill_switch));
  test('ConfigAck fixture matches ConfigAckPayload', () => assertConfigAck(fixtures.config_ack));
  test('OtaAck fixture matches OtaAckPayload', () => assertOtaAck(fixtures.ota_ack));
  test('FlagCacheSync fixture matches FlagCacheSyncPayload', () => assertFlagCacheSync(fixtures.flag_cache_sync));

  test('FlagSync field names match Rust FlagSyncPayload', () => {
    const required = ['flags', 'version'];
    required.forEach(f => expect(f in fixtures.flag_sync, `missing field: ${f}`).toBe(true));
  });

  test('ConfigPush field names match Rust ConfigPushPayload', () => {
    const required = ['fields', 'schema_version', 'sequence'];
    required.forEach(f => expect(f in fixtures.config_push, `missing field: ${f}`).toBe(true));
  });

  test('KillSwitch field names match Rust KillSwitchPayload', () => {
    const required = ['flag_name', 'active'];
    required.forEach(f => expect(f in fixtures.kill_switch, `missing field: ${f}`).toBe(true));
  });
});
