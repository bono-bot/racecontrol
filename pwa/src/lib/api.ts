const API_BASE =
  process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api/v1";

const GATEWAY_URL =
  process.env.NEXT_PUBLIC_GATEWAY_URL || "/api/payments";

declare global {
  interface Window {
    Razorpay: new (options: Record<string, unknown>) => {
      open: () => void;
      on: (event: string, handler: (response: unknown) => void) => void;
    };
  }
}

// ─── Auth helpers ──────────────────────────────────────────────────────────

let _redirecting = false;

export function getToken(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem("rp_token");
}

export function setToken(token: string) {
  localStorage.setItem("rp_token", token);
}

export function clearToken() {
  localStorage.removeItem("rp_token");
}

export function isLoggedIn(): boolean {
  return !!getToken();
}

function forceLogout() {
  if (_redirecting) return;
  _redirecting = true;
  clearToken();
  if (typeof window !== "undefined") {
    window.location.replace("/login");
  }
}

// ─── Fetch wrapper ─────────────────────────────────────────────────────────

async function fetchApi<T>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string>),
  };

  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const res = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers,
  });

  let data: unknown;
  try {
    data = await res.json();
  } catch {
    throw new Error(`HTTP ${res.status}: non-JSON response`);
  }

  // Auto-logout on JWT auth errors
  if (
    data &&
    typeof data === "object" &&
    "error" in data &&
    typeof (data as Record<string, unknown>).error === "string"
  ) {
    const err = (data as Record<string, unknown>).error as string;
    const hasRedirect = "_clear" in (data as Record<string, unknown>);
    if (err.includes("JWT decode error") || err.includes("Missing Authorization") || err === "session_expired" || hasRedirect) {
      forceLogout();
      return {} as T;
    }
  }

  return data as T;
}

// ─── Types ─────────────────────────────────────────────────────────────────

export interface CafeMenuItem {
  id: string;
  name: string;
  description: string | null;
  category_id: string;
  category_name: string;
  selling_price_paise: number;
  cost_price_paise: number;
  is_available: boolean;
  created_at: string | null;
  updated_at: string | null;
  image_path: string | null;
  // Stock fields (from Plan 01)
  is_countable: boolean;
  stock_quantity: number;
  out_of_stock: boolean;
}

export interface CafeMenuResponse {
  items: CafeMenuItem[];
  total: number;
  page: number;
}

export interface CafeOrderItem {
  item_id: string;
  quantity: number;
}

export interface CafeOrderRequest {
  driver_id: string;
  items: CafeOrderItem[];
}

export interface CafeOrderItemDetail {
  item_id: string;
  name: string;
  quantity: number;
  unit_price_paise: number;
  line_total_paise: number;
}

export interface ActivePromo {
  id: string;
  name: string;
  promo_type: "combo" | "happy_hour" | "gaming_bundle";
  config: Record<string, unknown>;
  stacking_group: string | null;
  time_label: string | null;
}

export interface CafeOrderResponse {
  order_id: string;
  receipt_number: string;
  wallet_txn_id: string;
  total_paise: number;
  discount_paise: number;
  applied_promo_id: string | null;
  applied_promo_name: string | null;
  new_balance_paise: number;
  items: CafeOrderItemDetail[];
}

export interface CafeOrderHistoryItem {
  id: string;
  receipt_number: string;
  items: CafeOrderItemDetail[];
  total_paise: number;
  status: string;
  created_at: string;
}

export interface CafeOrderHistoryResponse {
  orders: CafeOrderHistoryItem[];
}

export interface DriverProfile {
  id: string;
  customer_id: string | null;
  name: string;
  nickname: string | null;
  show_nickname_on_leaderboard: boolean;
  email: string | null;
  phone: string | null;
  total_laps: number;
  total_time_ms: number;
  has_used_trial: boolean;
  wallet_balance_paise: number;
  active_reservation: PodReservation | null;
}

export interface WalletInfo {
  driver_id: string;
  balance_paise: number;
  total_credited_paise: number;
  total_debited_paise: number;
  updated_at: string | null;
}

export interface WalletTransaction {
  id: string;
  driver_id: string;
  amount_paise: number;
  balance_after_paise: number;
  txn_type: string;
  reference_id: string | null;
  notes: string | null;
  staff_id: string | null;
  created_at: string;
}

export interface PodReservation {
  id: string;
  driver_id: string;
  pod_id: string;
  status: string;
  created_at: string;
  ended_at: string | null;
  last_activity_at: string | null;
}

export interface Experience {
  id: string;
  name: string;
  game: string;
  track: string;
  car: string;
  car_class: string | null;
  duration_minutes: number;
  start_type: string;
  sort_order: number;
}

export interface PricingTier {
  id: string;
  name: string;
  duration_minutes: number;
  price_paise: number;
  is_trial: boolean;
  sort_order: number;
}

export interface BillingSession {
  id: string;
  pod_id: string;
  allocated_seconds: number;
  driving_seconds: number;
  status: string;
  started_at: string | null;
  ended_at: string | null;
  custom_price_paise: number | null;
  discount_paise: number | null;
  original_price_paise: number | null;
  discount_reason: string | null;
}

export interface LapRecord {
  id: string;
  track: string;
  car: string;
  sim_type: string;
  lap_time_ms: number;
  lap_number?: number;
  sector1_ms: number | null;
  sector2_ms: number | null;
  sector3_ms: number | null;
  valid: boolean;
  created_at: string;
}

export interface CustomerStats {
  total_laps: number;
  total_time_ms: number;
  total_sessions: number;
  total_driving_seconds: number;
  favourite_car: string | null;
  personal_bests: number;
}

export interface SessionDetailSession {
  id: string;
  pod_id: string;
  pricing_tier_name: string;
  allocated_seconds: number;
  driving_seconds: number;
  status: string;
  price_paise: number;
  started_at: string | null;
  ended_at: string | null;
  experience_id: string | null;
  experience_name: string | null;
  car: string | null;
  track: string | null;
  sim_type: string | null;
  wallet_debit_paise: number | null;
  discount_paise: number | null;
  original_price_paise: number | null;
  discount_reason: string | null;
  refund_paise: number | null;
  total_laps: number;
  best_lap_ms: number | null;
  average_lap_ms: number | null;
  group_session_id: string | null;
  // Peak-end session experience fields (Phase 91)
  percentile_rank: number | null;
  percentile_text: string | null;
  is_new_pb: boolean;
  personal_best_ms: number | null;
  improvement_ms: number | null;
  peak_lap_number: number | null;
}

export interface SessionEvent {
  id: string;
  event_type: string;
  driving_seconds_at_event: number;
  metadata: string | null;
  created_at: string;
}

export interface SessionDetail {
  session: SessionDetailSession;
  laps: LapRecord[];
  events: SessionEvent[];
}

export interface PublicSessionSummary {
  driver_first_name: string;
  status: string;
  duration_seconds: number;
  pricing_tier: string;
  car: string | null;
  track: string | null;
  sim_type: string | null;
  best_lap_ms: number | null;
  total_laps: number;
  error?: string;
}

export interface TelemetryFrame {
  pod_id: string;
  driver_name: string;
  car: string;
  track: string;
  lap_number: number;
  lap_time_ms: number;
  sector: number;
  speed_kmh: number;
  throttle: number;
  brake: number;
  steering: number;
  gear: number;
  rpm: number;
  drs_active?: boolean;
  drs_available?: boolean;
  ers_deploy_mode?: number;
  ers_store_percent?: number;
  best_lap_ms?: number;
  current_lap_invalid?: boolean;
  sector1_ms?: number;
  sector2_ms?: number;
  sector3_ms?: number;
}

// ─── Assist State Types ──────────────────────────────────────────────────

export interface AssistState {
  abs: number;       // 0=off, 1-4=level
  tc: number;        // 0=off, 1-4=level
  auto_shifter: boolean;
  ffb_percent: number; // 10-100
}

// ─── AC Catalog Types ─────────────────────────────────────────────────────

export interface CatalogTrack {
  id: string;
  name: string;
  category: string;
  country?: string;
}

export interface CatalogCar {
  id: string;
  name: string;
  category: string;
}

export interface PresetEntry {
  id: string;
  name: string;
  tagline: string;
  car_id: string;
  car_name: string;
  track_id: string;
  track_name: string;
  session_type: string;
  difficulty: string;
  category: string;
  duration_hint: string;
  featured: boolean;
}

export interface ACCatalog {
  tracks: { featured: CatalogTrack[]; all: CatalogTrack[] };
  cars: { featured: CatalogCar[]; all: CatalogCar[] };
  categories: { tracks: string[]; cars: string[] };
  presets?: PresetEntry[];
}

export interface CustomBookingPayload {
  game: string;
  game_mode: string;
  track: string;
  car: string;
  difficulty: string;
  transmission: string;
  ffb?: string;
  session_type?: string;
}

// ─── Friends & Multiplayer Types ──────────────────────────────────────────

export interface FriendInfo {
  driver_id: string;
  name: string;
  customer_id: string | null;
  is_online: boolean;
  total_laps: number;
  total_time_ms: number;
  session_count: number;
}

export interface FriendRequestInfo {
  id: string;
  driver_id: string;
  driver_name: string;
  customer_id: string | null;
  direction: string;
  created_at: string;
}

export interface GroupSessionInfo {
  id: string;
  host_driver_id: string;
  host_name: string;
  experience_name: string;
  pricing_tier_name: string;
  shared_pin: string;
  status: string;
  members: GroupMemberInfo[];
  created_at: string;
  track?: string;
  car?: string;
  ai_count?: number;
  difficulty_tier?: string;
}

export interface GroupMemberInfo {
  driver_id: string;
  driver_name: string;
  customer_id: string | null;
  role: string;
  status: string;
  pod_id: string | null;
  pod_number: number | null;
}

// ─── Multiplayer Results Types ────────────────────────────────────────────

export interface MultiplayerResultInfo {
  position: number;
  driver_name: string;
  driver_id: string;
  best_lap_ms: number | null;
  total_time_ms: number | null;
  laps_completed: number;
  dnf: boolean;
}

// ─── Share Report Types ───────────────────────────────────────────────────

export interface ShareReport {
  driver_name: string;
  track: string;
  car: string;
  date: string | null;
  driving_time_seconds: number;
  driving_time_display: string;
  total_laps: number;
  valid_laps: number;
  best_lap_ms: number | null;
  best_lap_display: string | null;
  average_lap_ms: number | null;
  improvement_ms: number | null;
  consistency: { std_dev_ms: number; coefficient_of_variation: number; rating: string } | null;
  percentile_rank: number | null;
  percentile_text: string | null;
  track_record: { time_ms: number; holder: string; gap_ms: number | null } | null;
  personal_best_ms: number | null;
  is_new_pb: boolean;
  laps: { lap: number; time_ms: number; s1: number | null; s2: number | null; s3: number | null; valid: boolean }[];
  venue: string;
  tagline: string;
}

// ─── Passport & Badge Types ────────────────────────────────────────────────

export interface PassportTierItem {
  id: string;
  name: string;
  category: string;
  country?: string;
  driven: boolean;
  lap_count: number;
  best_lap_ms: number | null;
  first_driven_at: string | null;
}

export interface PassportTier {
  name: string;
  target: number;
  driven_count: number;
  items: PassportTierItem[];
}

export interface PassportCollection {
  total_driven: number;
  total_available: number;
  tiers: {
    starter: PassportTier;
    explorer: PassportTier;
    legend: PassportTier;
  };
  other: PassportTierItem[];
}

export interface PassportData {
  passport: {
    tracks: PassportCollection;
    cars: PassportCollection;
    summary: {
      unique_tracks: number;
      unique_cars: number;
      total_laps: number;
      streak_weeks: number;
      longest_streak: number | null;
      last_visit_date: string | null;
      grace_expires_date: string | null;
    };
  };
}

export interface Badge {
  id: string;
  name: string;
  description: string;
  category: string;
  icon: string;
  earned: boolean;
  earned_at?: string;
  progress?: number;
  target?: number;
}

export interface BadgesData {
  badges: {
    earned: Badge[];
    available: Badge[];
    total_earned: number;
    total_available: number;
  };
}

// ─── Package & Membership Types ──────────────────────────────────────────

export interface PackageInfo {
  id: string;
  name: string;
  description: string | null;
  num_rigs: number;
  duration_minutes: number;
  price_paise: number;
  price_display: string;
  includes_cafe: boolean;
  day_restriction: string | null;
  hour_start: number | null;
  hour_end: number | null;
}

export interface MembershipTier {
  id: string;
  name: string;
  hours_included: number;
  price_paise: number;
  price_display: string;
  perks: string[];
}

export interface MembershipInfo {
  id: string;
  tier_name: string;
  perks: string[];
  hours_used: number;
  hours_included: number;
  hours_remaining: number;
  expires_at: string;
  auto_renew: boolean;
  status: string;
}

// ─── Tournament Types ─────────────────────────────────────────────────────

export interface TournamentInfo {
  id: string;
  name: string;
  description: string | null;
  track: string;
  car: string;
  format: string;
  max_participants: number;
  entry_fee_display: string;
  prize_pool_display: string;
  status: string;
  event_date: string | null;
  is_registered: boolean;
}

export interface CompareLapsResult {
  track: string;
  car: string;
  my_best: { time_ms: number; s1_ms: number | null; s2_ms: number | null; s3_ms: number | null };
  reference: { driver: string; time_ms: number; s1_ms: number | null; s2_ms: number | null; s3_ms: number | null } | null;
  sector_analysis: { s1_delta_ms: number | null; s2_delta_ms: number | null; s3_delta_ms: number | null; weakest_sector: string | null; total_delta_ms: number } | null;
  recent_trend: number[];
  improving: boolean | null;
  tip: string | null;
}

// ─── Active Session Event Types ──────────────────────────────────────────

export interface ActiveSessionEvent {
  type: string;
  lap_id: string;
  lap_time_ms: number;
  track: string;
  car: string;
  at: string;
}

export interface ActiveSessionEventsResponse {
  events: ActiveSessionEvent[];
  error?: string;
}

// ─── Remote Reservation Types ─────────────────────────────────────────────

export interface RemoteReservation {
  id: string;
  pin: string;
  status: string; // pending_debit | confirmed | expired | cancelled | failed
  experience_name: string;
  price_paise: number;
  expires_at: string;
  created_at: string;
  debit_status?: string;
}

// ─── API calls ─────────────────────────────────────────────────────────────

export const api = {
  // Auth
  login: (phone: string) =>
    fetchApi<{ status?: string; delivered?: boolean; error?: string }>("/customer/login", {
      method: "POST",
      body: JSON.stringify({ phone }),
    }),

  resendOtp: (phone: string) =>
    fetchApi<{ status?: string; delivered?: boolean; error?: string }>("/customer/resend-otp", {
      method: "POST",
      body: JSON.stringify({ phone }),
    }),

  verifyOtp: (phone: string, otp: string) =>
    fetchApi<{ status?: string; token?: string; error?: string }>(
      "/customer/verify-otp",
      {
        method: "POST",
        body: JSON.stringify({ phone, otp }),
      }
    ),

  // Customer data
  profile: () =>
    fetchApi<{ driver?: DriverProfile; error?: string }>("/customer/profile"),

  sessions: () =>
    fetchApi<{ sessions?: BillingSession[]; error?: string }>(
      "/customer/sessions"
    ),

  sessionDetail: (id: string) =>
    fetchApi<SessionDetail & { error?: string }>(
      `/customer/sessions/${encodeURIComponent(id)}`
    ),

  laps: () =>
    fetchApi<{ laps?: LapRecord[]; error?: string }>("/customer/laps"),

  stats: () =>
    fetchApi<{ stats?: CustomerStats; error?: string }>("/customer/stats"),

  // QR auth
  validateQr: (qrToken: string, driverId: string) =>
    fetchApi<{
      status?: string;
      billing_session_id?: string;
      error?: string;
    }>("/auth/validate-qr", {
      method: "POST",
      body: JSON.stringify({ qr_token: qrToken, driver_id: driverId }),
    }),

  // Registration
  register: (data: {
    name: string;
    nickname?: string;
    dob: string;
    email?: string;
    waiver_consent: boolean;
    signature_data?: string;
    guardian_name?: string;
    guardian_phone?: string;
  }) =>
    fetchApi<{ status?: string; driver_id?: string; is_minor?: boolean; error?: string }>(
      "/customer/register",
      {
        method: "POST",
        body: JSON.stringify(data),
      }
    ),

  updateProfile: (data: { nickname?: string; show_nickname_on_leaderboard?: boolean }) =>
    fetchApi<{ status?: string; error?: string }>("/customer/profile", {
      method: "PUT",
      body: JSON.stringify(data),
    }),

  // Leaderboard (public)
  leaderboard: (track: string) =>
    fetchApi<{ leaderboard?: unknown; error?: string }>(
      `/leaderboard/${encodeURIComponent(track)}`
    ),

  // Venue info (public)
  venue: () => fetchApi<{ name: string; location: string }>("/venue"),

  // Wallet
  wallet: () =>
    fetchApi<{ wallet?: WalletInfo; error?: string }>("/customer/wallet"),

  walletTransactions: (limit = 50) =>
    fetchApi<{ transactions?: WalletTransaction[]; error?: string }>(
      `/customer/wallet/transactions?limit=${limit}`
    ),

  bonusTiers: () =>
    fetchApi<{
      tiers?: { id: string; min_paise: number; bonus_pct: number; sort_order: number }[];
    }>("/wallet/bonus-tiers"),

  createTopupOrder: async (amount_paise: number) => {
    const token = getToken();
    const res = await fetch(`${GATEWAY_URL}/create-order`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      body: JSON.stringify({ amount_paise }),
    });
    if (!res.ok) throw new Error(`Order creation failed: ${res.status}`);
    return res.json() as Promise<{
      order_id: string;
      amount: number;
      currency: string;
      key_id: string;
      error?: string;
    }>;
  },

  // Experiences & Booking
  experiences: () =>
    fetchApi<{
      experiences?: Experience[];
      pricing_tiers?: PricingTier[];
      error?: string;
    }>("/customer/experiences"),

  acCatalog: () =>
    fetchApi<ACCatalog & { error?: string }>("/customer/ac/catalog"),

  bookSession: (experience_id: string, pricing_tier_id: string) =>
    fetchApi<{
      status?: string;
      reservation_id?: string;
      pod_id?: string;
      pod_number?: number;
      pin?: string;
      allocated_seconds?: number;
      wallet_debit_paise?: number;
      error?: string;
      balance_paise?: number;
      required_paise?: number;
    }>("/customer/book", {
      method: "POST",
      body: JSON.stringify({ experience_id, pricing_tier_id }),
    }),

  bookCustom: (pricing_tier_id: string, custom: CustomBookingPayload, coupon_code?: string) =>
    fetchApi<{
      status?: string;
      reservation_id?: string;
      pod_id?: string;
      pod_number?: number;
      pin?: string;
      allocated_seconds?: number;
      wallet_debit_paise?: number;
      discount_paise?: number;
      original_price_paise?: number;
      discount_reason?: string;
      error?: string;
      balance_paise?: number;
      required_paise?: number;
    }>("/customer/book", {
      method: "POST",
      body: JSON.stringify({ pricing_tier_id, custom, coupon_code: coupon_code || undefined }),
    }),

  activeReservation: () =>
    fetchApi<{
      reservation?: PodReservation | null;
      pod_number?: number;
      active_billing?: BillingSession | null;
      error?: string;
    }>("/customer/active-reservation"),

  endReservation: () =>
    fetchApi<{ status?: string; error?: string }>("/customer/end-reservation", {
      method: "POST",
    }),

  continueSession: (experience_id: string, pricing_tier_id: string) =>
    fetchApi<{
      status?: string;
      billing_session_id?: string;
      reservation_id?: string;
      pod_id?: string;
      error?: string;
    }>("/customer/continue-session", {
      method: "POST",
      body: JSON.stringify({ experience_id, pricing_tier_id }),
    }),

  // Telemetry
  telemetry: () =>
    fetchApi<{ frame?: TelemetryFrame; error?: string }>("/customer/telemetry"),

  // AI Chat
  aiChat: (message: string, history: { role: string; content: string }[]) =>
    fetchApi<{ reply?: string; model?: string; error?: string }>(
      "/customer/ai/chat",
      {
        method: "POST",
        body: JSON.stringify({ message, history }),
      }
    ),

  // Terminal
  terminalAuth: (pin: string) =>
    fetchApi<{ session?: string; expires_at?: string; error?: string }>(
      "/terminal/auth",
      {
        method: "POST",
        body: JSON.stringify({ pin }),
      }
    ),

  terminalSubmit: (cmd: string, timeout_ms = 30000, session?: string) =>
    fetchApi<{ status?: string; id?: string; error?: string }>(
      "/terminal/commands",
      {
        method: "POST",
        body: JSON.stringify({ cmd, timeout_ms }),
        headers: session
          ? { "x-terminal-session": session }
          : { "x-terminal-secret": "rp-terminal-2026" },
      }
    ),

  terminalList: (limit = 50, session?: string) =>
    fetchApi<{ commands?: TerminalCommand[]; error?: string }>(
      `/terminal/commands?limit=${limit}`,
      {
        headers: session
          ? { "x-terminal-session": session }
          : { "x-terminal-secret": "rp-terminal-2026" },
      }
    ),

  // Friends
  friends: () =>
    fetchApi<{ friends?: FriendInfo[]; error?: string }>("/customer/friends"),

  friendRequests: () =>
    fetchApi<{
      incoming?: FriendRequestInfo[];
      outgoing?: FriendRequestInfo[];
      error?: string;
    }>("/customer/friends/requests"),

  sendFriendRequest: (identifier: string) =>
    fetchApi<{ request_id?: string; error?: string }>(
      "/customer/friends/request",
      {
        method: "POST",
        body: JSON.stringify({ identifier }),
      }
    ),

  acceptFriendRequest: (requestId: string) =>
    fetchApi<{ status?: string; error?: string }>(
      `/customer/friends/request/${encodeURIComponent(requestId)}/accept`,
      { method: "POST" }
    ),

  rejectFriendRequest: (requestId: string) =>
    fetchApi<{ status?: string; error?: string }>(
      `/customer/friends/request/${encodeURIComponent(requestId)}/reject`,
      { method: "POST" }
    ),

  removeFriend: (friendDriverId: string) =>
    fetchApi<{ status?: string; error?: string }>(
      `/customer/friends/${encodeURIComponent(friendDriverId)}`,
      { method: "DELETE" }
    ),

  setPresence: (presence: string) =>
    fetchApi<{ status?: string; error?: string }>("/customer/presence", {
      method: "PUT",
      body: JSON.stringify({ presence }),
    }),

  // Multiplayer
  bookMultiplayer: (
    pricing_tier_id: string,
    friend_ids: string[],
    experience_id?: string,
    custom?: CustomBookingPayload,
  ) =>
    fetchApi<{ group_session?: GroupSessionInfo; error?: string }>(
      "/customer/book-multiplayer",
      {
        method: "POST",
        body: JSON.stringify({ pricing_tier_id, friend_ids, ...(experience_id ? { experience_id } : {}), ...(custom ? { custom } : {}) }),
      }
    ),

  multiplayerResults: (groupSessionId: string) =>
    fetchApi<{ results?: MultiplayerResultInfo[]; error?: string }>(
      `/customer/multiplayer-results/${encodeURIComponent(groupSessionId)}`
    ),

  groupSession: () =>
    fetchApi<{ group_session?: GroupSessionInfo | null; error?: string }>(
      "/customer/group-session"
    ),

  acceptGroupInvite: (groupSessionId: string) =>
    fetchApi<{
      status?: string;
      member?: GroupMemberInfo;
      error?: string;
    }>(`/customer/group-session/${encodeURIComponent(groupSessionId)}/accept`, {
      method: "POST",
    }),

  declineGroupInvite: (groupSessionId: string) =>
    fetchApi<{ status?: string; error?: string }>(
      `/customer/group-session/${encodeURIComponent(groupSessionId)}/decline`,
      { method: "POST" }
    ),

  // Session share report
  sessionShare: (id: string) =>
    fetchApi<{ share_report?: ShareReport; error?: string }>(
      `/customer/sessions/${encodeURIComponent(id)}/share`
    ),

  // Referrals
  referralCode: () =>
    fetchApi<{ referral_code?: string | null; successful_referrals?: number; error?: string }>(
      "/customer/referral-code"
    ),

  generateReferralCode: () =>
    fetchApi<{ referral_code?: string; error?: string }>(
      "/customer/referral-code/generate",
      { method: "POST" }
    ),

  redeemReferral: (code: string) =>
    fetchApi<{ ok?: boolean; message?: string; error?: string }>(
      "/customer/redeem-referral",
      { method: "POST", body: JSON.stringify({ code }) }
    ),

  // Coupons
  applyCoupon: (code: string) =>
    fetchApi<{ valid?: boolean; coupon_id?: string; coupon_type?: string; value?: number; description?: string; error?: string }>(
      "/customer/apply-coupon",
      { method: "POST", body: JSON.stringify({ code }) }
    ),

  // Packages
  packages: () =>
    fetchApi<{ packages?: PackageInfo[]; error?: string }>("/customer/packages"),

  // Memberships
  membership: () =>
    fetchApi<{ membership?: MembershipInfo | null; available_tiers?: MembershipTier[]; error?: string }>(
      "/customer/membership"
    ),

  subscribeMembership: (tier_id: string) =>
    fetchApi<{ ok?: boolean; membership_id?: string; tier_name?: string; message?: string; error?: string }>(
      "/customer/membership/subscribe",
      { method: "POST", body: JSON.stringify({ tier_id }) }
    ),

  // Tournaments
  tournaments: () =>
    fetchApi<{ tournaments?: TournamentInfo[]; error?: string }>("/customer/tournaments"),

  registerTournament: (id: string) =>
    fetchApi<{ ok?: boolean; registration_id?: string; error?: string }>(
      `/customer/tournaments/${encodeURIComponent(id)}/register`,
      { method: "POST" }
    ),

  // Coaching / Compare
  compareLaps: (track: string, car: string, compareTo?: string) =>
    fetchApi<CompareLapsResult & { error?: string }>(
      `/customer/compare-laps?track=${encodeURIComponent(track)}&car=${encodeURIComponent(car)}${compareTo ? `&compare_to=${encodeURIComponent(compareTo)}` : ""}`
    ),

  // Driving Passport
  passport: () =>
    fetchApi<PassportData & { error?: string }>("/customer/passport"),

  // Badges
  badges: () =>
    fetchApi<BadgesData & { error?: string }>("/customer/badges"),

  // Mid-session controls
  setAssist: (podId: string, assistType: string, enabled: boolean) =>
    fetchApi<{ ok: boolean }>(`/pods/${podId}/assists`, {
      method: "POST",
      body: JSON.stringify({ assist_type: assistType, enabled }),
    }),

  setFfbGain: (podId: string, percent: number) =>
    fetchApi<{ ok: boolean; ffb_percent?: number }>(`/pods/${podId}/ffb`, {
      method: "POST",
      body: JSON.stringify({ percent }),
    }),

  getAssistState: (podId: string) =>
    fetchApi<{ ok: boolean; abs?: number; tc?: number; auto_shifter?: boolean; ffb_percent?: number }>(
      `/pods/${podId}/assist-state`
    ),

  activeSessionEvents: (since: string): Promise<ActiveSessionEventsResponse> =>
    fetchApi<ActiveSessionEventsResponse>(`/customer/active-session/events?since=${encodeURIComponent(since)}`),

  // Remote reservations (cloud booking with PIN)
  createReservation: (experience_id: string, pricing_tier_id: string) =>
    fetchApi<{
      status?: string;
      reservation_id?: string;
      pin?: string;
      expires_at?: string;
      experience_name?: string;
      price_paise?: number;
      error?: string;
    }>("/customer/reservation", {
      method: "POST",
      body: JSON.stringify({ experience_id, pricing_tier_id }),
    }),

  getReservation: () =>
    fetchApi<{
      reservation?: RemoteReservation | null;
      error?: string;
    }>("/customer/reservation"),

  cancelReservation: () =>
    fetchApi<{ status?: string; refund_paise?: number; error?: string }>(
      "/customer/reservation",
      { method: "DELETE" }
    ),

  modifyReservation: (experience_id: string, pricing_tier_id: string) =>
    fetchApi<{
      status?: string;
      reservation_id?: string;
      pin?: string;
      experience_name?: string;
      error?: string;
    }>("/customer/reservation", {
      method: "PUT",
      body: JSON.stringify({ experience_id, pricing_tier_id }),
    }),

  // Cafe ordering
  placeCafeOrder: (items: CafeOrderItem[]) =>
    fetchApi<CafeOrderResponse | { error: string }>("/customer/cafe/orders", {
      method: "POST",
      body: JSON.stringify({ driver_id: "", items }),
    }),

  getCafeOrderHistory: () =>
    fetchApi<CafeOrderHistoryResponse>("/customer/cafe/orders/history"),
};

// ─── Leaderboard Types ────────────────────────────────────────────────────

export interface LeaderboardEntry {
  position: number;
  driver: string;
  car: string;
  best_lap_ms: number;
  is_personal_best: boolean;
  is_track_record: boolean;
  lap_id?: string;
}

// ─── Lap Telemetry Types ──────────────────────────────────────────────────

export interface LapTelemetrySample {
  offset_ms: number;
  speed: number | null;
  throttle: number | null;
  brake: number | null;
  steering: number | null;
  gear: number | null;
  rpm: number | null;
}

export interface LapTelemetryData {
  lap_id: string;
  track: string;
  car: string;
  sim_type: string;
  lap_time_ms: number;
  sector1_ms: number | null;
  sector2_ms: number | null;
  sector3_ms: number | null;
  samples: LapTelemetrySample[];
  sample_count: number;
  error?: string;
}

// ─── Public API (no auth) ─────────────────────────────────────────────────

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api/v1";

export function getImageBaseUrl(): string {
  const base = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api/v1";
  return base.replace(/\/api\/v1$/, "");
}

export const publicApi = {
  leaderboard: () =>
    fetch(`${API_BASE_URL}/public/leaderboard`).then(r => r.json()),

  trackLeaderboard: (track: string, params?: { sim_type?: string; car?: string; show_invalid?: boolean }) => {
    const qs = new URLSearchParams();
    if (params?.sim_type) qs.set("sim_type", params.sim_type);
    if (params?.car) qs.set("car", params.car);
    if (params?.show_invalid) qs.set("show_invalid", "true");
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return fetch(`${API_BASE_URL}/public/leaderboard/${encodeURIComponent(track)}${suffix}`).then(r => r.json());
  },

  circuitRecords: (params?: { sim_type?: string }) => {
    const qs = new URLSearchParams();
    if (params?.sim_type) qs.set("sim_type", params.sim_type);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return fetch(`${API_BASE_URL}/public/circuit-records${suffix}`).then(r => r.json());
  },

  vehicleRecords: (car: string, params?: { sim_type?: string }) => {
    const qs = new URLSearchParams();
    if (params?.sim_type) qs.set("sim_type", params.sim_type);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return fetch(`${API_BASE_URL}/public/vehicle-records/${encodeURIComponent(car)}${suffix}`).then(r => r.json());
  },

  searchDrivers: (name: string) =>
    fetch(`${API_BASE_URL}/public/drivers?name=${encodeURIComponent(name)}`).then(r => r.json()),

  driverProfile: (id: string) =>
    fetch(`${API_BASE_URL}/public/drivers/${encodeURIComponent(id)}`).then(r => r.json()),

  timeTrial: () =>
    fetch(`${API_BASE_URL}/public/time-trial`).then(r => r.json()),

  lapTelemetry: (lapId: string, resolution?: string) => {
    const qs = new URLSearchParams();
    if (resolution) qs.set("resolution", resolution);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return fetch(`${API_BASE_URL}/public/laps/${encodeURIComponent(lapId)}/telemetry${suffix}`)
      .then(r => r.json()) as Promise<LapTelemetryData>;
  },

  sessionSummary: (id: string) =>
    fetch(`${API_BASE_URL}/public/sessions/${encodeURIComponent(id)}`)
      .then(r => r.json()) as Promise<PublicSessionSummary>,

  cafeMenu: () =>
    fetch(`${API_BASE_URL}/cafe/menu`).then(r => r.json()) as Promise<CafeMenuResponse>,

  activePromos: () =>
    fetch(`${API_BASE_URL}/cafe/promos/active`).then(r => r.json()) as Promise<ActivePromo[]>,
};

// ─── Mesh Intelligence / Diagnosis Types (read-only for PWA staff) ───────────

export interface PodHealth {
  pod_id: string;
  pod_number: number;
  seconds_since_heartbeat: number;
  health: string;
  status: string;
}

export interface DebugIncident {
  id: string;
  pod_id?: string;
  category: string;
  description: string;
  status: string;
  created_at: string;
}

export interface DebugActivityData {
  pod_health: PodHealth[];
  billing_events: { id: string; event_type: string; created_at: string; pod_id?: string }[];
  game_events: { id: string; pod_id: string; event_type: string; created_at: string }[];
}

export interface MeshSolution {
  id: string;
  problem_key: string;
  root_cause: string;
  fix_type: string;
  status: string;
  success_count: number;
  fail_count: number;
  confidence: number;
  cost_to_diagnose: number;
  diagnosis_tier: string;
  source_node: string;
  created_at: string;
  tags?: string[];
}

export interface MeshStats {
  total_solutions: number;
  candidates: number;
  fleet_verified: number;
  hardened: number;
  total_incidents: number;
  total_cost: number;
  avg_confidence: number;
}

export interface PodDiagnosticEvent {
  timestamp: string;
  trigger: string;
  tier: number;
  outcome: string;
  action: string;
  root_cause: string;
  fix_type: string;
  confidence: number;
  fix_applied: boolean;
  source: string;
}

// ─── Staff fetch wrapper (uses sessionStorage staff token, NOT customer token) ───

function getStaffToken(): string | null {
  if (typeof window === "undefined") return null;
  return sessionStorage.getItem("pwa_staff_token");
}

async function fetchStaffApi<T>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const token = getStaffToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string>),
  };

  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const res = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers,
  });

  if (!res.ok) {
    const text = await res.text().catch(() => "");
    if (res.status === 401) {
      // Staff token expired — clear and force re-auth
      sessionStorage.removeItem("pwa_staff_token");
      sessionStorage.removeItem("pwa_staff_name");
    }
    throw new Error(`Staff API ${res.status}: ${path} — ${text.slice(0, 200)}`);
  }

  return res.json();
}

// Staff diagnosis API (read-only — no incident creation or fix application from PWA)
// Uses fetchStaffApi which reads staff JWT from sessionStorage, NOT customer token
export const staffDiagnosisApi = {
  debugActivity: (hours?: number) =>
    fetchStaffApi<DebugActivityData>(`/debug/activity${hours ? `?hours=${hours}` : ""}`),

  listIncidents: (status?: string) =>
    fetchStaffApi<{ incidents: DebugIncident[] }>(`/debug/incidents${status ? `?status=${status}` : ""}`),

  podDiagnosticEvents: (podId: string, limit?: number) =>
    fetchStaffApi<{ events: PodDiagnosticEvent[] }>(
      `/debug/pod-events/${podId}${limit ? `?limit=${limit}` : ""}`
    ),

  meshSolutions: () =>
    fetchStaffApi<{ solutions: MeshSolution[] }>("/mesh/solutions"),

  meshStats: () =>
    fetchStaffApi<MeshStats>("/mesh/stats"),
};

// Staff PIN validation for PWA staff mode
// Uses regular fetchApi (no auth needed for PIN validation — rate-limited on backend)
export const staffAuth = {
  validatePin: (pin: string) =>
    fetchApi<{ status?: string; error?: string; token?: string; staff_name?: string }>(
      "/staff/validate-pin",
      { method: "POST", body: JSON.stringify({ pin }) }
    ),
};

export interface TerminalCommand {
  id: string;
  cmd: string;
  status: string;
  exit_code: number | null;
  stdout: string | null;
  stderr: string | null;
  timeout_ms: number;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
}
