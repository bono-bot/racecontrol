const TOKEN_KEY = "rp_staff_jwt";

// ACCEPTED RISK: JWT stored in localStorage. This is a LAN-only kiosk system
// with no public internet exposure. Moving to httpOnly cookies requires server-side
// session management (Set-Cookie on login, cookie-based auth middleware in Axum).
// TODO: Migrate to httpOnly cookies when server auth is refactored.

export function getToken(): string | null {
  if (typeof window === "undefined") return null;
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    // localStorage unavailable (quota exceeded, kiosk restriction, SecurityError)
    return null;
  }
}

export function setToken(token: string): void {
  if (typeof window === "undefined") return;
  try {
    localStorage.setItem(TOKEN_KEY, token);
  } catch {
    // Quota exceeded or storage unavailable — token won't persist across reloads
    console.warn("[auth] Failed to save token to localStorage");
  }
}

export function clearToken(): void {
  if (typeof window === "undefined") return;
  try {
    localStorage.removeItem(TOKEN_KEY);
  } catch {
    // Storage unavailable — token may persist but will expire via JWT exp
  }
}

export function isAuthenticated(): boolean {
  const token = getToken();
  if (!token) return false;
  try {
    const payload = JSON.parse(atob(token.split(".")[1]));
    return payload.exp * 1000 > Date.now();
  } catch {
    return false;
  }
}
