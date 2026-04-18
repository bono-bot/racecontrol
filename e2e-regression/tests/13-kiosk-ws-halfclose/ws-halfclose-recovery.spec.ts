// ═══════════════════════════════════════════════════════════════
// 13 — Kiosk WS Half-Close Recovery. Pins DE-1 fix `2c27e2fc`.
//
// Bug: send_task in ws/dashboard_handler.rs exited early while the
// outer loop blocked forever on half-duplex WS → receiver_count
// drained → "channel closed" broadcast failures → kiosk UI froze.
// Fix added a WARN "send_task exited early — aborting recv_task"
// and aborts recv_task so the half-socket cleans up.
//
// This spec asserts:
//   (a) dashboard_ws_churn.connects_per_min < 10 (stable connections)
//   (b) "channel closed" error count over 60s sample is 0
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { RCApiClient } from '../../fixtures/api-client';
import { SERVER_IP, STAFF_PIN } from '../../fixtures/test-data';

const api = new RCApiClient();

test.describe('13 — Kiosk WS half-close', () => {
  test.beforeAll(async () => {
    await api.login(STAFF_PIN);
  });

  test('dashboard_ws_churn < 10 connects/min', async () => {
    const resp = await fetch(`http://${SERVER_IP}:8080/api/v1/fleet/health`, {
      headers: { 'Authorization': `Bearer ${(api as any).token}` },
    });
    expect(resp.ok).toBe(true);
    const health: any = await resp.json();

    const churn = health.dashboard_ws_churn || {};
    const connectsPerMin = churn.connects_per_min ?? 0;

    console.log(`\n═══ WS HALF-CLOSE STABILITY ═══`);
    console.log(`dashboard_ws_churn.connects_per_min: ${connectsPerMin}`);
    console.log(`dashboard_ws_churn full:`, JSON.stringify(churn));

    // 10/min was the 2026-04-03 regression threshold that caught the
    // stale-admin-build disconnect loop. Same metric pins DE-1.
    expect(connectsPerMin).toBeLessThan(10);
  });

  test('no "channel closed" broadcast errors in last 60s', async () => {
    // Query the server's structured JSONL logs via the log API
    const resp = await fetch(`http://${SERVER_IP}:8080/api/v1/logs?window_secs=60&level=warn`, {
      headers: { 'Authorization': `Bearer ${(api as any).token}` },
    });

    if (!resp.ok) {
      test.skip(true, `log API unreachable (${resp.status}) — pin deferred`);
    }

    const data: any = await resp.json();
    const lines: string[] = data.lines || data.entries || [];

    const channelClosed = lines.filter(l =>
      typeof l === 'string' ? l.includes('channel closed') : JSON.stringify(l).includes('channel closed')
    );

    console.log(`\n log lines in last 60s: ${lines.length}`);
    console.log(` "channel closed" matches: ${channelClosed.length}`);
    if (channelClosed.length > 0) {
      console.log(' first 3:', channelClosed.slice(0, 3));
    }

    // After 2c27e2fc this should be 0 in steady state. If half-close
    // regresses, this count will climb as dashboards lose their
    // broadcast receiver.
    expect(channelClosed.length).toBe(0);
  });
});
