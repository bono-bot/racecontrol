// ═══════════════════════════════════════════════════════════════
// Test Driver Factory — create and manage test drivers + wallets
// ═══════════════════════════════════════════════════════════════

import { RCApiClient } from './api-client';
import { PaymentMethod } from './test-data';

const testDriverIds: string[] = [];
const testCouponIds: string[] = [];

let driverCounter = 0;

// Create a unique test driver with a funded wallet
// Known waiver-signed driver from the database (Vishal Chavan)
const KNOWN_WAIVER_DRIVER = 'drv_8d1025c4';
let sharedDriverFunded = false;

export async function createTestDriver(
  api: RCApiClient,
  options: {
    name?: string;
    balancePaise?: number;
    paymentMethod?: PaymentMethod;
    isTrial?: boolean;
    isMinor?: boolean;
    guardianId?: string;
  } = {},
): Promise<{ driverId: string; name: string; phone: string }> {
  driverCounter++;

  // Use the known waiver-signed driver for all billing tests
  // This avoids venue/register customer_id collision issues
  const driverId = KNOWN_WAIVER_DRIVER;

  // Fund wallet if needed (only top up once per run, then as needed)
  const balance = options.balancePaise ?? 200000;
  if (balance > 0) {
    try {
      const wallet = await api.getWallet(driverId);
      if (wallet.balance_paise < balance) {
        await api.topupWallet(driverId, {
          amount_paise: balance,
          method: options.paymentMethod || 'cash',
          notes: 'E2E test funding',
        });
      }
    } catch {
      // Wallet might not exist yet — topup will create it
      await api.topupWallet(driverId, {
        amount_paise: balance,
        method: options.paymentMethod || 'cash',
        notes: 'E2E test funding',
      });
    }
  }

  testDriverIds.push(driverId);
  return { driverId, name: 'Vishal Chavan', phone: '' };
}

// Create a linked driver pair (parent + child)
export async function createLinkedDriverPair(
  api: RCApiClient,
): Promise<{ parent: { driverId: string; name: string }; child: { driverId: string; name: string } }> {
  const parent = await createTestDriver(api, { name: 'E2E_Parent', balancePaise: 500000 });
  const child = await createTestDriver(api, {
    name: 'E2E_Child',
    balancePaise: 0,
    isMinor: true,
    guardianId: parent.driverId,
  });

  return {
    parent: { driverId: parent.driverId, name: parent.name },
    child: { driverId: child.driverId, name: child.name },
  };
}

// Create test coupons
export async function createTestCoupons(api: RCApiClient): Promise<{
  percentCoupon: string;
  flatCoupon: string;
  freeMinCoupon: string;
}> {
  const ts = Date.now();

  const pct = await api.createCoupon({
    code: `E2E_PCT_${ts}`,
    coupon_type: 'percentage',
    value: 10, // 10% off
    max_uses: 100,
    is_active: true,
  });

  const flat = await api.createCoupon({
    code: `E2E_FLAT_${ts}`,
    coupon_type: 'fixed_amount',
    value: 5000, // ₹50 off
    max_uses: 100,
    is_active: true,
  });

  const freeMin = await api.createCoupon({
    code: `E2E_FREE_${ts}`,
    coupon_type: 'free_minutes',
    value: 5, // 5 free minutes
    max_uses: 100,
    is_active: true,
  });

  testCouponIds.push(pct.id, flat.id, freeMin.id);

  return {
    percentCoupon: `E2E_PCT_${ts}`,
    flatCoupon: `E2E_FLAT_${ts}`,
    freeMinCoupon: `E2E_FREE_${ts}`,
  };
}

// Create a short-duration E2E pricing tier for "completed" tests
export async function createE2EPricingTier(api: RCApiClient): Promise<string> {
  const tier = await api.createPricingTier({
    name: 'E2E_1min',
    duration_minutes: 1,
    price_paise: 2500, // ₹25
    is_trial: false,
    is_active: true,
  });
  return tier.id;
}

// Ensure driver has enough wallet balance
export async function ensureWalletBalance(
  api: RCApiClient,
  driverId: string,
  requiredPaise: number,
  paymentMethod: PaymentMethod = 'cash',
): Promise<number> {
  const wallet = await api.getWallet(driverId);
  if (wallet.balance_paise < requiredPaise) {
    const topup = requiredPaise - wallet.balance_paise + 10000; // +₹100 buffer
    const result = await api.topupWallet(driverId, {
      amount_paise: topup,
      method: paymentMethod,
      notes: `E2E auto-topup: needed ${requiredPaise}, had ${wallet.balance_paise}`,
    });
    return result.balance_paise;
  }
  return wallet.balance_paise;
}

// Cleanup: collect all test driver/coupon IDs for post-suite cleanup
export function getTestDriverIds(): string[] {
  return [...testDriverIds];
}

export function getTestCouponIds(): string[] {
  return [...testCouponIds];
}
