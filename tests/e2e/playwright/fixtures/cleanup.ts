import { test as base, request } from '@playwright/test';

const API_BASE = process.env.RC_API_URL ?? 'http://192.168.31.23:8080';
const TEST_POD = process.env.TEST_POD_ID ?? 'pod-8';

export const test = base.extend<{ ensureClean: void }>({
  ensureClean: [async ({}, use) => {
    const ctx = await request.newContext({ baseURL: API_BASE });

    // Stop any active games on test pod
    try {
      const gamesRes = await ctx.get('/api/v1/games/active');
      if (gamesRes.ok()) {
        const games = await gamesRes.json();
        for (const game of games ?? []) {
          if (game.pod_id === TEST_POD) {
            await ctx.post('/api/v1/games/stop', { data: { pod_id: TEST_POD } });
          }
        }
      }
    } catch { /* server may be unreachable — smoke tests will fail on their own */ }

    // End any active billing sessions on test pod
    try {
      const billingRes = await ctx.get('/api/v1/billing/active');
      if (billingRes.ok()) {
        const data = await billingRes.json();
        const sessions = data?.sessions ?? data ?? [];
        for (const session of Array.isArray(sessions) ? sessions : []) {
          if (session.pod_id === TEST_POD) {
            await ctx.post(`/api/v1/billing/${session.id}/stop`, {});
          }
        }
      }
    } catch { /* same — idempotent, failures are non-fatal */ }

    await ctx.dispose();
    await use();

    // No teardown needed — smoke tests are read-only after cleanup
  }, { auto: true }],
});

export { expect } from '@playwright/test';
