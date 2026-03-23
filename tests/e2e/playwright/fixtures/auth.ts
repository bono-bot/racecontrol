import { test as base, expect, request } from '@playwright/test';

const API_BASE = process.env.RC_API_URL ?? 'http://192.168.31.23:8080';

/**
 * Shared fixture that provides a staff JWT for authenticated tests.
 * Uses the employee daily-pin endpoint to get a valid token.
 */
export const test = base.extend<{ staffToken: string; apiContext: ReturnType<typeof request.newContext> extends Promise<infer T> ? T : never }>({
  staffToken: async ({}, use) => {
    const ctx = await request.newContext({ baseURL: API_BASE });
    let token = '';
    try {
      // Get daily PIN and validate to get staff JWT
      const pinRes = await ctx.get('/api/v1/employee/daily-pin');
      if (pinRes.ok()) {
        const { pin } = await pinRes.json();
        const authRes = await ctx.post('/api/v1/staff/validate-pin', {
          data: { pin },
        });
        if (authRes.ok()) {
          const body = await authRes.json();
          token = body.token ?? '';
        }
      }
    } catch {
      // Auth may fail if server is down — tests will handle gracefully
    }
    await ctx.dispose();
    await use(token);
  },

  apiContext: async ({ staffToken }, use) => {
    const ctx = await request.newContext({
      baseURL: API_BASE,
      extraHTTPHeaders: staffToken
        ? { Authorization: `Bearer ${staffToken}` }
        : {},
    });
    await use(ctx);
    await ctx.dispose();
  },
});

export { expect };
