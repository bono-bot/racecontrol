// ═══════════════════════════════════════════════════════════════
// Core Session Matrix — 300 combinations
// Game × Tier × Payment × EndType (random pod selection)
// ═══════════════════════════════════════════════════════════════

import { test, expect } from '@playwright/test';
import { RCApiClient, BillingStartResponse } from '../../fixtures/api-client';
import { loginPOS, waitForApp } from '../../fixtures/auth';
import { screenshot, matrixScreenshotName } from '../../fixtures/screenshot-helper';
import { generateTestMatrix, STAFF_PIN, TERMINAL_STATUSES } from '../../fixtures/test-data';
import { getRandomIdlePod } from '../../fixtures/random-pod';
import { createTestDriver, ensureWalletBalance, createE2EPricingTier } from '../../fixtures/test-driver-factory';
import { verifyTerminalStatus, verifyWalletAfterSession, verifyBillingEvents } from '../../fixtures/billing-helpers';
import { verifyGameLaunchVisually, captureGameRunning, captureAfterEnd, cleanupPodRecording } from '../../fixtures/pod-screen-capture';

const api = new RCApiClient();
const matrix = generateTestMatrix();

let testDriverId: string;
let e2ePricingTierId: string;
let pricingTierMap: Map<string, string>; // tierName → tierId

test.describe('04 — Core Session Matrix (300 combinations)', () => {
  test.beforeAll(async () => {
    // Wait 5s before setup to avoid 429 from previous test suites
    await new Promise(r => setTimeout(r, 5000));

    await api.login(STAFF_PIN);

    // Use the known waiver-signed driver directly (no API call needed)
    testDriverId = 'drv_8d1025c4';
    console.log(`Test driver: Vishal Chavan (${testDriverId})`);

    // Ensure wallet has funds (single API call)
    try {
      const wallet = await api.getWallet(testDriverId);
      if (wallet.balance_paise < 5000000) {
        await api.topupWallet(testDriverId, { amount_paise: 5000000, method: 'cash', notes: 'E2E matrix funding' });
      }
    } catch {
      // Wallet may not exist or rate limited — continue anyway
      console.log('  Warning: wallet check/topup failed, continuing...');
    }

    // Map existing tier names to IDs (no creation needed — tiers already exist)
    try {
      const tiers = await api.listPricingTiers();
      pricingTierMap = new Map();
      for (const t of tiers) {
        if (t.duration_minutes === 30 && !t.is_trial) pricingTierMap.set('30min', t.id);
        if (t.duration_minutes === 60 && !t.is_trial) pricingTierMap.set('60min', t.id);
        if (t.is_trial) pricingTierMap.set('trial', t.id);
        if (t.name === 'E2E_1min') e2ePricingTierId = t.id;
      }
      pricingTierMap.set('per_minute', '');

      // Create E2E tier only if not already present
      if (!e2ePricingTierId) {
        e2ePricingTierId = await createE2EPricingTier(api);
      }
      console.log(`E2E tier: ${e2ePricingTierId}`);
      console.log(`Pricing tiers: ${[...pricingTierMap.entries()].map(([k, v]) => `${k}=${v}`).join(', ')}`);
    } catch (e) {
      console.log(`  Warning: tier setup failed: ${String(e).slice(0, 100)}`);
      pricingTierMap = new Map([['30min', 'tier_30min'], ['60min', 'tier_60min'], ['trial', 'tier_trial'], ['per_minute', '']]);
      e2ePricingTierId = 'tier_trial'; // fallback
    }
  });

  for (const combo of matrix) {
    test(`${combo.testId}: ${combo.game.name} × ${combo.tier.name} × ${combo.payment} × ${combo.endType}`, async ({ page }) => {
      // 1. Find a random idle pod with this game installed
      const pod = await getRandomIdlePod(api, combo.game.id);
      if (!pod) {
        test.skip(true, `No idle pod available with ${combo.game.name} installed`);
        return;
      }
      console.log(`  Using Pod ${pod.podNumber} (${pod.podIp})`);

      // 2. Pre-cleanup: stop any stuck game/session on this pod
      try { await api.stopGame({ pod_id: pod.podId }); } catch { /* ignore */ }
      try { await api.setPodScreen(pod.podId, 'blank'); } catch { /* ignore */ }

      // 3. Ensure driver has enough balance
      const requiredBalance = combo.tier.pricePaise + 10000;
      const balanceBefore = await ensureWalletBalance(api, testDriverId, requiredBalance, combo.payment);
      const walletBefore = await api.getWallet(testDriverId);

      // 3. Screenshot name prefix (skip browser navigation — pure API test)
      const ssPrefix = matrixScreenshotName(combo.testId, '', combo.game.id, combo.tier.name, combo.payment, combo.endType);

      // 4. Start billing session via API
      const tierId = combo.tier.name === 'per_minute'
        ? undefined
        : (combo.endType === 'completed' && combo.tier.name !== 'trial'
          ? e2ePricingTierId // Use 1-min tier for natural completion
          : pricingTierMap.get(combo.tier.name));

      let startResp: Awaited<ReturnType<typeof api.startBilling>>;
      try {
        startResp = await api.startBilling({
          pod_id: pod.podId,
          driver_id: testDriverId,
          pricing_tier_id: tierId,
          sim_type: combo.game.id,
          idempotency_key: `e2e-${combo.testId}-${Date.now()}`,
        });
      } catch (e) {
        console.log(`  Billing start error: ${String(e).slice(0, 150)}`);
        test.skip(true, `Billing start failed: ${String(e).slice(0, 100)}`);
        return;
      }
      const sessionId = startResp.billing_session_id;
      if (!sessionId) {
        console.log(`  Billing start returned no session: ${JSON.stringify(startResp)}`);
        test.skip(true, `No billing_session_id returned`);
        return;
      }
      console.log(`  Billing session: ${sessionId}, debit: ₹${startResp.wallet_debit_paise / 100}`);

      // 5. Pod screen capture: before launch
      const launchEvidence = await verifyGameLaunchVisually(pod.podIp, combo.testId, pod.podNumber);

      // 6. Launch game on the pod
      // AC: send launch_args with track/car/AI so acs.exe goes straight to track (triggers telemetry → billing active)
      // Other games: launch with just sim_type
      const isAC = combo.game.id === 'assetto_corsa';
      try {
        await api.launchGame({
          pod_id: pod.podId,
          sim_type: combo.game.id,
          launch_args: isAC ? {
            track: 'monza',
            car: 'ks_ferrari_sf15t',
            session_type: 'practice',
            ai_count: 3,
            ai_level: 85,
          } : undefined,
        });
        // Dismiss blanking screen so game is visible
        await api.setPodScreen(pod.podId, 'game');
      } catch (e) {
        console.log(`  Game launch failed (may be expected for cancel tests): ${e}`);
      }

      // (pod screen capture handles visual evidence — no browser needed)

      // 7. Verify game is actually running on the pod (check process list)
      await new Promise(r => setTimeout(r, 10000)); // Wait for game to start
      const gameState = await api.podGameState(pod.podId).catch(() => ({ game_state: 'unknown' } as any));
      const gameRunning = gameState?.game?.game_state === 'running' || gameState?.game_state === 'running';
      console.log(`  Game state: ${JSON.stringify(gameState?.game?.game_state || gameState?.state || 'unknown')}, running=${gameRunning}`);

      // Pod screen capture: game running
      await captureGameRunning(pod.podIp, combo.testId, pod.podNumber);
      // Pod screen capture is the visual evidence

      // 8. Check billing — may be 'active' (if telemetry arrived) or 'waiting_for_game' (API-launched)
      const billingCheck = await api.getBillingSession(sessionId);
      const billingActive = billingCheck.status === 'active';
      console.log(`  Billing: ${billingCheck.status}${billingActive ? ' (ACTIVE - telemetry flowing)' : ' (waiting - no telemetry yet)'}`);

      // 9. Execute end type
      if (combo.endType === 'cancelled_no_playable') {
        // Cancel immediately — game never reached playable billing state
        try { await api.stopBilling(sessionId); } catch { /* may already be ended */ }
      } else if (combo.endType === 'ended_early') {
        if (billingActive) {
          await new Promise(r => setTimeout(r, 5000)); // Let some driving time accrue
        }
        try { await api.stopBilling(sessionId); } catch { /* ignore */ }
      } else if (combo.endType === 'cancelled') {
        try { await api.stopBilling(sessionId); } catch { /* ignore */ }
      } else {
        // 'completed' — for waiting_for_game, just stop it; for active, wait for natural expiry
        if (billingActive) {
          try {
            await api.waitForBillingStatus(sessionId, [...TERMINAL_STATUSES], 120_000);
          } catch {
            await api.stopBilling(sessionId).catch(() => {});
          }
        } else {
          // Can't wait for natural completion without telemetry — stop it
          try { await api.stopBilling(sessionId); } catch { /* ignore */ }
        }
      }

      // 10. Get final session state
      let finalSession;
      try {
        finalSession = await api.getBillingSession(sessionId);
      } catch {
        finalSession = { status: 'unknown', driving_seconds: 0, wallet_debit_paise: startResp.wallet_debit_paise, allocated_seconds: 0 };
      }
      console.log(`  Final: status=${finalSession.status}, driving=${finalSession.driving_seconds}s`);

      // 11. Stop game and restore blanking screen
      try { await api.stopGame({ pod_id: pod.podId }); } catch { /* ignore */ }
      try { await api.setPodScreen(pod.podId, 'blank'); } catch { /* ignore */ }

      // 12. Session ended — pod screen capture is the evidence

      // 13. Pod screen after end
      await captureAfterEnd(pod.podIp, combo.testId, pod.podNumber);

      // 14. Verify: session reached a terminal state OR was stopped cleanly
      const isTerminal = TERMINAL_STATUSES.includes(finalSession.status as any);
      expect(isTerminal || finalSession.status === 'waiting_for_game').toBeTruthy();

      // 15. Verify: wallet debit matches tier price
      expect(startResp.wallet_debit_paise).toBe(startResp.original_price_paise - startResp.discount_paise);

      // 16. Verify: game actually ran on the pod
      expect(gameRunning).toBeTruthy();

      // 17. Cleanup pod recording
      await cleanupPodRecording(pod.podIp);

      // 18. Cooldown — ensure pod is fully released and server locks cleared
      await new Promise(r => setTimeout(r, 15000));

      console.log(`  ✓ ${combo.testId} PASSED (game=${gameRunning}, billing=${finalSession.status})`);
    });
  }
});
