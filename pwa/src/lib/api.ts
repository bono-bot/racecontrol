const API_BASE =
  process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api/v1";

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
}

export interface LapRecord {
  id: string;
  track: string;
  car: string;
  sim_type: string;
  lap_time_ms: number;
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
  refund_paise: number | null;
  total_laps: number;
  best_lap_ms: number | null;
  average_lap_ms: number | null;
}

export interface SessionDetail {
  session: SessionDetailSession;
  laps: LapRecord[];
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

export interface ACCatalog {
  tracks: { featured: CatalogTrack[]; all: CatalogTrack[] };
  cars: { featured: CatalogCar[]; all: CatalogCar[] };
  categories: { tracks: string[]; cars: string[] };
}

export interface CustomBookingPayload {
  game: string;
  game_mode: string;
  track: string;
  car: string;
  difficulty: string;
  transmission: string;
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
  hour_restriction: string | null;
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

// ─── API calls ─────────────────────────────────────────────────────────────

export const api = {
  // Auth
  login: (phone: string) =>
    fetchApi<{ status?: string; error?: string }>("/customer/login", {
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

  bookCustom: (pricing_tier_id: string, custom: CustomBookingPayload) =>
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
      body: JSON.stringify({ pricing_tier_id, custom }),
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
};

// ─── Public API (no auth) ─────────────────────────────────────────────────

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api/v1";

export const publicApi = {
  leaderboard: () =>
    fetch(`${API_BASE_URL}/public/leaderboard`).then(r => r.json()),

  trackLeaderboard: (track: string) =>
    fetch(`${API_BASE_URL}/public/leaderboard/${encodeURIComponent(track)}`).then(r => r.json()),

  timeTrial: () =>
    fetch(`${API_BASE_URL}/public/time-trial`).then(r => r.json()),
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
