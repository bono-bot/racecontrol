// ─── Pure formatting functions ────────────────────────────────────────────
// Extracted for testability. Used across multiple kiosk pages.

/** Format lap time in milliseconds to M:SS.mmm */
export function formatLapTime(ms: number): string {
  if (ms <= 0) return "--:--.---";
  const totalSec = ms / 1000;
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toFixed(3).padStart(6, "0")}`;
}

/** Format seconds to M:SS countdown timer */
export function formatTimer(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

/** Format seconds to HH:MM:SS session timer */
export function formatSessionTimer(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/** Format uptime seconds to Xh Ym */
export function formatUptime(secs: number | null | undefined): string {
  if (secs == null) return "--";
  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor((secs % 3600) / 60);
  return `${hours}h ${minutes}m`;
}

/** Short game label from sim_type string */
export function gameLabel(simType: string): string {
  const map: Record<string, string> = {
    assetto_corsa: "AC",
    ac: "AC",
    assetto_corsa_evo: "ACE",
    assetto_corsa_rally: "ACR",
    f1_25: "F1",
    f1: "F1",
    iracing: "iR",
    le_mans_ultimate: "LMU",
    lmu: "LMU",
    forza: "FRZ",
    forza_horizon_5: "FH5",
  };
  return map[simType] || simType.toUpperCase().slice(0, 3);
}

/** Format ISO timestamp to IST time string */
export function formatTimeIST(isoStr: string): string {
  try {
    const d = new Date(isoStr);
    if (isNaN(d.getTime())) return "--:--:--";
    return d.toLocaleTimeString("en-IN", { timeZone: "Asia/Kolkata", hour12: false });
  } catch {
    return "--:--:--";
  }
}
