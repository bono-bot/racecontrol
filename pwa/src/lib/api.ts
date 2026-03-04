const API_BASE =
  process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api/v1";

// ─── Auth helpers ──────────────────────────────────────────────────────────

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

  return res.json();
}

// ─── Types ─────────────────────────────────────────────────────────────────

export interface DriverProfile {
  id: string;
  customer_id: string | null;
  name: string;
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

export interface SessionDetail {
  session: BillingSession;
  laps: LapRecord[];
  track: string | null;
  car: string | null;
  total_laps: number;
  best_lap_ms: number | null;
  avg_lap_ms: number | null;
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

  bookSession: (experience_id: string, pricing_tier_id: string) =>
    fetchApi<{
      status?: string;
      reservation_id?: string;
      pod_id?: string;
      pod_number?: number;
      qr_token?: string;
      wallet_debit_paise?: number;
      error?: string;
      balance_paise?: number;
      required_paise?: number;
    }>("/customer/book", {
      method: "POST",
      body: JSON.stringify({ experience_id, pricing_tier_id }),
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
