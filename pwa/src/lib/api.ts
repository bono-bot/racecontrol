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
  name: string;
  email: string | null;
  phone: string | null;
  total_laps: number;
  total_time_ms: number;
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

  // Leaderboard (public)
  leaderboard: (track: string) =>
    fetchApi<{ leaderboard?: unknown; error?: string }>(
      `/leaderboard/${encodeURIComponent(track)}`
    ),

  // Venue info (public)
  venue: () => fetchApi<{ name: string; location: string }>("/venue"),
};
