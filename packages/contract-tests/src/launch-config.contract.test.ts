import { describe, test, expect } from 'vitest';
import fixtures from './fixtures/launch-config.json';

/**
 * Launch boundary FIXTURE SHAPE-LOCK (racecontrol side). Source of truth:
 * crates/rc-common/src/launch_contract.rs.
 *
 * WHAT THIS IS NOT: this is NOT a cross-language CONSUMER contract test. It imports
 * the fixture and asserts its shape with inline checks; it runs NO production parser.
 * There is no production TS parser for GameConfig yet — the zod type does not exist,
 * and racecontrol-agent.ts is the OpenRouter agent, not the launch agent. So this
 * guards only that the FIXTURE stays well-formed; it does NOT prove the kiosk/PWA
 * consumer parses these bytes correctly. If the kiosk still expects ai_difficulty:
 * "easy" tomorrow, these assertions stay green — the drift would NOT be caught here.
 *
 * The real cross-language guard on THIS side is the Rust serde round-trip
 * (crates/rc-common/tests/launch_contract_vectors.rs), which pins the Rust->wire
 * direction exactly. The missing half — production zod .parse() over these exact
 * bytes, deep-equal to the typed value — must live in rp-v2-apps CI (separate PR),
 * because that is the repo where the kiosk consumer is edited and where its CI runs.
 * Until that lands, the cross-language boundary is HALF-pinned (Rust->wire only).
 */

const GAME_TYPES = ['AssettCorsa', 'F125', 'IRacing', 'LeManUltimate'];
const AC_SESSION_TYPES = ['Practice', 'Race', 'Hotlap', 'TimeAttack', 'Drift'];

const isU8 = (n: unknown): boolean =>
  typeof n === 'number' && Number.isInteger(n) && n >= 0 && n <= 255;

/** Externally-tagged enum: exactly one key, drawn from `allowed`. */
function expectExternallyTagged(obj: unknown, allowed: string[], label: string): string {
  expect(obj !== null && typeof obj === 'object', `${label} must be an object`).toBe(true);
  const keys = Object.keys(obj as Record<string, unknown>);
  expect(keys.length, `${label} must have exactly one variant key`).toBe(1);
  expect(allowed.includes(keys[0]), `${label} variant '${keys[0]}' not in ${allowed.join('|')}`).toBe(true);
  return keys[0];
}

const launchRequests = fixtures.launch_request as Record<string, any>;
const launchResults = fixtures.launch_result as Record<string, any>;
const gameEvents = fixtures.game_event as Record<string, any>;

describe('Launch boundary fixture shape-lock (NOT a consumer contract — see rp-v2-apps PR for production-zod parse)', () => {
  test('fixture groups are present and non-empty', () => {
    expect(Object.keys(launchRequests).length).toBeGreaterThan(0);
    expect(Object.keys(launchResults).length).toBeGreaterThan(0);
    expect(Object.keys(gameEvents).length).toBeGreaterThan(0);
  });

  describe.each(Object.entries(launchRequests))('LaunchRequest[%s]', (_name: string, req: any) => {
    test('top-level fields + types match LaunchRequest', () => {
      for (const f of ['game', 'pod_id', 'driver_id', 'driver_name', 'method', 'config']) {
        expect(f in req, `missing field: ${f}`).toBe(true);
      }
      expect(GAME_TYPES.includes(req.game), `game '${req.game}' not a GameType`).toBe(true);
      expect(isU8(req.pod_id), 'pod_id must be u8').toBe(true);
      expect(typeof req.driver_id, 'driver_id must be number (i64)').toBe('number');
      expect(typeof req.driver_name, 'driver_name must be string').toBe('string');
    });

    test('method is externally-tagged StaffKiosk|PwaPin', () => {
      const variant = expectExternallyTagged(req.method, ['StaffKiosk', 'PwaPin'], 'method');
      if (variant === 'StaffKiosk') expect(typeof req.method.StaffKiosk.staff_id).toBe('string');
      else expect(typeof req.method.PwaPin.pin).toBe('string');
    });

    test('config variant matches game; AC ai fields are NUMERIC not string', () => {
      const variant = expectExternallyTagged(
        req.config,
        GAME_TYPES,
        'config',
      );
      expect(variant, 'config variant must match game').toBe(req.game);

      if (variant === 'AssettCorsa') {
        const ac = req.config.AssettCorsa;
        expect(typeof ac.track, 'track must be string').toBe('string');
        expect(typeof ac.car, 'car must be string').toBe('string');
        expect(AC_SESSION_TYPES.includes(ac.session_type), `session_type '${ac.session_type}' invalid`).toBe(true);
        // THE drift guard — ai_level / ai_count are u8 on the wire, never "easy".
        expect(isU8(ac.ai_level), 'ai_level must be numeric u8, never a string').toBe(true);
        expect(isU8(ac.ai_count), 'ai_count must be numeric u8, never a string').toBe(true);
        // server: null | { ip, http_port, password? }
        if (ac.server !== null) {
          expect(typeof ac.server.ip).toBe('string');
          expect(isU8(ac.server.http_port) || (Number.isInteger(ac.server.http_port) && ac.server.http_port <= 65535)).toBe(true);
          if (ac.server.password !== null) expect(typeof ac.server.password).toBe('string');
        }
      }
    });
  });

  describe.each(Object.entries(launchResults))('LaunchResult[%s]', (_name: string, res: any) => {
    test('pod_id + game + externally-tagged outcome', () => {
      expect(isU8(res.pod_id), 'pod_id must be u8').toBe(true);
      expect(GAME_TYPES.includes(res.game)).toBe(true);
      const variant = expectExternallyTagged(res.outcome, ['Spawned', 'Failed'], 'outcome');
      if (variant === 'Spawned') {
        const pid = res.outcome.Spawned.pid;
        expect(pid === null || Number.isInteger(pid), 'pid must be number|null').toBe(true);
      } else {
        expect(typeof res.outcome.Failed.reason).toBe('string');
      }
    });
  });

  describe.each(Object.entries(gameEvents))('GameEvent[%s]', (_name: string, ev: any) => {
    test('externally-tagged Started|Crashed|Ended with typed payload', () => {
      const variant = expectExternallyTagged(ev, ['Started', 'Crashed', 'Ended'], 'GameEvent');
      const payload = ev[variant];
      expect(isU8(payload.pod_id), 'pod_id must be u8').toBe(true);
      expect(GAME_TYPES.includes(payload.game)).toBe(true);
      if (variant === 'Started') expect(Number.isInteger(payload.pid)).toBe(true);
      if (variant === 'Crashed') expect(typeof payload.reason).toBe('string');
      if (variant === 'Ended') expect(Number.isInteger(payload.duration_secs)).toBe(true);
    });
  });
});
