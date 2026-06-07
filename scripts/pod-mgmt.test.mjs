// node --test scripts/pod-mgmt.test.mjs
// Unit tests for the venue->transport->pod resolver (pure logic).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  loadRegistry,
  resolveVenue,
  resolveTransport,
  resolvePodTarget,
  resolveFallbackTarget,
  listTargets,
} from './pod-mgmt.mjs';

// Synthetic venues for sold/heart + error cases (independent of the shipped file).
const soldVenue = {
  venue_type: 'sold',
  pod_transport: 'heart-exec',
  fallback_transport: 'heart-exec',
  venue_heart: { url: 'https://rp-demo.racecontrol.in/' }, // trailing slash on purpose
  pods: { 'pod-1': { id: 'pod-1' } },
};

test('real registry loads + rp-vlm is own/tailscale-ssh', () => {
  const reg = loadRegistry();
  const v = resolveVenue(reg, 'rp-vlm');
  assert.equal(v.venue_type, 'own');
  assert.equal(resolveTransport(v), 'tailscale-ssh');
});

test('unknown venue throws; templates are not selectable', () => {
  const reg = loadRegistry();
  assert.throws(() => resolveVenue(reg, 'nope'), /unknown venue/);
  assert.throws(() => resolveVenue(reg, '_template-sold'), /template/);
});

test('own pod resolves to a direct SSH target with the right host/user', () => {
  const v = resolveVenue(loadRegistry(), 'rp-vlm');
  const t = resolvePodTarget(v, 'pod-8');
  assert.equal(t.kind, 'ssh');
  assert.equal(t.host, '100.98.67.67');
  assert.equal(t.user, 'ADMIN');
  assert.equal(t.transport, 'tailscale-ssh');
});

test('own fallback resolves to the audited rc-sentry channel', () => {
  const v = resolveVenue(loadRegistry(), 'rp-vlm');
  const f = resolveFallbackTarget(v, 'pod-8');
  assert.equal(f.kind, 'rc-sentry');
  assert.match(f.url, /^http:\/\/100\.98\.67\.67:8091$/);
});

test('listTargets returns all 8 rp-vlm pods, all SSH', () => {
  const v = resolveVenue(loadRegistry(), 'rp-vlm');
  const all = listTargets(v);
  assert.equal(all.length, 8);
  assert.ok(all.every((t) => t.kind === 'ssh'));
});

test('unknown pod throws', () => {
  const v = resolveVenue(loadRegistry(), 'rp-vlm');
  assert.throws(() => resolvePodTarget(v, 'pod-99'), /unknown pod/);
});

test('sold venue resolves via the heart pod_exec proxy (trailing slash normalized)', () => {
  const t = resolvePodTarget(soldVenue, 'pod-1');
  assert.equal(t.kind, 'heart');
  assert.equal(t.url, 'https://rp-demo.racecontrol.in/pods/pod-1/exec');
});

test('heart-exec without venue_heart.url throws (no silent wrong target)', () => {
  const broken = { pod_transport: 'heart-exec', venue_heart: {}, pods: { 'pod-1': {} } };
  assert.throws(() => resolvePodTarget(broken, 'pod-1'), /venue_heart\.url/);
});

test('unknown transport throws', () => {
  const bad = { pod_transport: 'carrier-pigeon', pods: { 'pod-1': { tailnet: '1.2.3.4' } } };
  assert.throws(() => resolvePodTarget(bad, 'pod-1'), /unknown transport/);
});
