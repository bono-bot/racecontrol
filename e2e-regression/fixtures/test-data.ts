// ═══════════════════════════════════════════════════════════════
// Test Data Constants — All axes of the regression matrix
// ═══════════════════════════════════════════════════════════════

export const SERVER_IP = '192.168.31.23';
export const API_BASE = `http://${SERVER_IP}:8080/api/v1`;
export const POS_BASE = `http://${SERVER_IP}:3200`;
export const KIOSK_BASE = `http://${SERVER_IP}:3300`;

export const STAFF_PIN = '0009'; // Chavan Vishal
export const ADMIN_PIN = '261121'; // Master admin PIN

// Pod IPs — for rc-agent :8090 screen capture
export const POD_IPS: Record<number, string> = {
  1: '192.168.31.89',
  2: '192.168.31.33',
  3: '192.168.31.28',
  4: '192.168.31.88',
  5: '192.168.31.86',
  6: '192.168.31.87',
  7: '192.168.31.38',
  8: '192.168.31.91',
};

// ─── Games (SimType enum) ────────────────────────────────────
export const GAMES = [
  { id: 'assetto_corsa', name: 'Assetto Corsa', udpPort: 9996 },
  { id: 'assetto_corsa_evo', name: 'Assetto Corsa Evo', udpPort: 9996 },
  { id: 'f1_25', name: 'F1 25', udpPort: 20777 },
  { id: 'iracing', name: 'iRacing', udpPort: 6789 },
  { id: 'le_mans_ultimate', name: 'Le Mans Ultimate', udpPort: 5555 },
  { id: 'forza', name: 'Forza Motorsport', udpPort: 5300 },
  { id: 'forza_horizon_5', name: 'Forza Horizon 5', udpPort: 5300 },
  { id: 'assetto_corsa_rally', name: 'EA SPORTS WRC', udpPort: 9996 },
] as const;

export type GameId = typeof GAMES[number]['id'];

// ─── Pricing Tiers ───────────────────────────────────────────
export const TIERS = [
  { name: '30min', durationMin: 30, pricePaise: 75000, mode: 'package' as const, isTrial: false },
  { name: '60min', durationMin: 60, pricePaise: 90000, mode: 'package' as const, isTrial: false },
  { name: 'trial', durationMin: 5, pricePaise: 0, mode: 'package' as const, isTrial: true },
  { name: 'per_minute', durationMin: 0, pricePaise: 0, mode: 'per_minute' as const, isTrial: false, ratePerMin: 2500 },
] as const;

export type TierName = typeof TIERS[number]['name'];

// Per-minute tiered rates
export const PER_MINUTE_RATES = [
  { rangeStart: 0, rangeEnd: 30, ratePerMinPaise: 2500 },   // 0-30 min
  { rangeStart: 31, rangeEnd: 60, ratePerMinPaise: 2000 },  // 31-60 min
  { rangeStart: 61, rangeEnd: Infinity, ratePerMinPaise: 1500 }, // 60+ min
];

// ─── Payment Methods (for wallet topup) ──────────────────────
export const PAYMENTS = ['cash', 'upi', 'card'] as const;
export type PaymentMethod = typeof PAYMENTS[number];

// ─── Session End Types ───────────────────────────────────────
export const END_TYPES = ['completed', 'ended_early', 'cancelled', 'cancelled_no_playable'] as const;
export type EndType = typeof END_TYPES[number];

// ─── Pause Types ─────────────────────────────────────────────
export const PAUSE_TYPES = ['manual', 'game_pause', 'disconnect', 'crash_recovery'] as const;
export type PauseType = typeof PAUSE_TYPES[number];

// ─── Session Types ───────────────────────────────────────────
export const SESSION_TYPES = ['Practice', 'Qualifying', 'Race', 'Hotlap'] as const;

// ─── Billing Session Statuses ────────────────────────────────
export const BILLING_STATUSES = [
  'pending', 'waiting_for_game', 'active',
  'paused_manual', 'paused_disconnect', 'paused_game_pause', 'paused_crash_recovery',
  'completed', 'ended_early', 'cancelled', 'cancelled_no_playable',
] as const;

export const TERMINAL_STATUSES = ['completed', 'ended_early', 'cancelled', 'cancelled_no_playable'] as const;

// ─── Coupon Types ────────────────────────────────────────────
export const COUPON_TYPES = ['percentage', 'fixed_amount', 'free_minutes'] as const;

// ─── Featured Tracks (subset for testing) ────────────────────
export const TEST_TRACKS = [
  'spa', 'monza', 'silverstone', 'red_bull_ring', 'monaco',
  'nordschleife', 'mugello', 'brands_hatch',
];

// ─── Featured Cars (subset for testing) ──────────────────────
export const TEST_CARS = [
  'ks_ferrari_sf15t', 'bmw_m8_lms', 'ks_porsche_911_gt3_r',
  'ks_lamborghini_huracan_gt3', 'ks_mclaren_p1',
];

// ─── Test combination generator ──────────────────────────────
export interface TestCombination {
  game: typeof GAMES[number];
  tier: typeof TIERS[number];
  payment: PaymentMethod;
  endType: EndType;
  testId: string;
}

export function generateTestMatrix(): TestCombination[] {
  const combos: TestCombination[] = [];
  let idx = 0;

  for (const game of GAMES) {
    for (const tier of TIERS) {
      // Trial only valid for assetto_corsa
      if (tier.isTrial && game.id !== 'assetto_corsa') continue;

      for (const payment of PAYMENTS) {
        for (const endType of END_TYPES) {
          idx++;
          combos.push({
            game,
            tier,
            payment,
            endType,
            testId: `M-${String(idx).padStart(3, '0')}`,
          });
        }
      }
    }
  }

  return combos;
}

// ─── POS Sidebar Pages (for button audit) ────────────────────
export const POS_PAGES = [
  { path: '/', name: 'Live-Overview' },
  { path: '/pods', name: 'Pods' },
  { path: '/games', name: 'Games' },
  { path: '/telemetry', name: 'Telemetry' },
  { path: '/ac-lan', name: 'AC-LAN' },
  { path: '/ac-sessions', name: 'AC-Results' },
  { path: '/sessions', name: 'Sessions' },
  { path: '/drivers', name: 'Drivers' },
  { path: '/leaderboards', name: 'Leaderboards' },
  { path: '/events', name: 'Events' },
  { path: '/billing', name: 'Billing' },
  { path: '/billing/pricing', name: 'Pricing' },
  { path: '/billing/history', name: 'History' },
  { path: '/bookings', name: 'Bookings' },
  { path: '/ai', name: 'AI-Insights' },
  { path: '/cameras', name: 'Cameras' },
  { path: '/cameras/playback', name: 'Playback' },
  { path: '/cafe', name: 'Cafe-Menu' },
  { path: '/settings', name: 'Settings' },
  { path: '/presenter', name: 'Presenter' },
  { path: '/kiosk', name: 'Kiosk-Mode' },
];

// ─── Kiosk Pages (for button audit) ──────────────────────────
export const KIOSK_PAGES = [
  { path: '/kiosk/', name: 'Lock-Screen' },
  { path: '/kiosk/staff', name: 'Staff-Login' },
  { path: '/kiosk/spectator', name: 'Spectator' },
  { path: '/kiosk/register', name: 'Register' },
];

export const KIOSK_STAFF_PAGES = [
  { path: '/kiosk/staff', name: 'Staff-Terminal' },
  { path: '/kiosk/control', name: 'Control' },
  { path: '/kiosk/fleet', name: 'Fleet' },
  { path: '/kiosk/settings', name: 'Settings' },
  { path: '/kiosk/debug', name: 'Debug' },
];
