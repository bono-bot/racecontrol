import { describe, test, expect } from 'vitest';
import type { BillingSessionStatus, GameState } from '@racingpoint/types';
import wsDashboardFixture from './fixtures/ws-dashboard.json';

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

const VALID_GAME_STATES: GameState[] = [
  'idle',
  'launching',
  'loading',
  'running',
  'stopping',
  'error',
];

describe('DashboardEvent::BillingTick — payload shape validation', () => {
  test('BillingTick fixture has required fields', () => {
    const tick = wsDashboardFixture.billing_tick;
    expect(typeof tick.pod_id).toBe('string');
    expect(typeof tick.session_id).toBe('string');
    expect(VALID_BILLING_STATUSES).toContain(tick.status);
    expect(typeof tick.remaining_seconds).toBe('number');
    expect(typeof tick.elapsed_seconds).toBe('number');
  });

  test('BillingTick with active status validates correctly', () => {
    const tick = wsDashboardFixture.billing_tick;
    expect(tick.status).toBe('active');
    expect(tick.remaining_seconds).toBeGreaterThan(0);
    expect(tick.elapsed_seconds).toBeGreaterThanOrEqual(0);
  });

  test('BillingTick with waiting_for_game status validates correctly', () => {
    const tick = wsDashboardFixture.billing_tick_waiting;
    expect(typeof tick.pod_id).toBe('string');
    expect(typeof tick.session_id).toBe('string');
    expect(VALID_BILLING_STATUSES).toContain(tick.status);
    expect(tick.status).toBe('waiting_for_game');
    expect(typeof tick.remaining_seconds).toBe('number');
    expect(typeof tick.elapsed_seconds).toBe('number');
    expect(tick.elapsed_seconds).toBe(0);
  });

  test('pod_id is non-empty string in BillingTick', () => {
    expect(wsDashboardFixture.billing_tick.pod_id.length).toBeGreaterThan(0);
    expect(wsDashboardFixture.billing_tick_waiting.pod_id.length).toBeGreaterThan(0);
  });
});

describe('DashboardEvent::GameStateChanged — payload shape validation', () => {
  test('GameStateChanged fixture has required fields', () => {
    const event = wsDashboardFixture.game_state_changed;
    expect(typeof event.pod_id).toBe('string');
    expect(VALID_GAME_STATES).toContain(event.game_state);
  });

  test('GameStateChanged with running state validates correctly', () => {
    const event = wsDashboardFixture.game_state_changed;
    expect(event.pod_id).toBe('pod-1');
    expect(event.game_state).toBe('running');
  });

  test('GameState loading variant validates correctly', () => {
    const event = wsDashboardFixture.game_state_loading;
    expect(typeof event.pod_id).toBe('string');
    expect(VALID_GAME_STATES).toContain(event.game_state);
    expect(event.game_state).toBe('loading');
  });

  test('pod_id is non-empty string in GameStateChanged', () => {
    expect(wsDashboardFixture.game_state_changed.pod_id.length).toBeGreaterThan(0);
    expect(wsDashboardFixture.game_state_loading.pod_id.length).toBeGreaterThan(0);
  });
});
