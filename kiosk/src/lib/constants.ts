// ─── Shared Constants ────────────────────────────────────────────────────────
// Game data derived from gameDisplayInfo.ts — single source of truth.
// At runtime, call loadGameCatalog() to fetch from API and update dynamically.

import { GAME_DISPLAY, mergeApiCatalog, type GameDisplayEntry } from "./gameDisplayInfo";
import { api } from "./api";

// ─── Dynamic game catalog ───────────────────────────────────────────────────
// Starts with fallback data, updated when API responds.

let _gameDisplay: Record<string, GameDisplayEntry> = { ...GAME_DISPLAY };
let _catalogLoaded = false;

/** Fetch game catalog from API and merge into display data. Safe to call multiple times. */
export async function loadGameCatalog(): Promise<void> {
  if (_catalogLoaded) return;
  try {
    const res = await api.gamesCatalog();
    if (res.games?.length) {
      _gameDisplay = mergeApiCatalog(res.games);
      _catalogLoaded = true;
    }
  } catch {
    // API unreachable — use fallback. Will retry on next call.
  }
}

/** Current game display map (fallback until API loads). */
export function getGameDisplay(): Record<string, GameDisplayEntry> {
  return _gameDisplay;
}

// Derive GAMES list from current display data
export function getGames(): { id: string; name: string; enabled: boolean }[] {
  return Object.entries(_gameDisplay).map(([id, entry]) => ({
    id,
    name: entry.name,
    enabled: true,
  }));
}

// Static exports for backward compatibility (uses fallback until loadGameCatalog resolves)
export const GAMES = Object.entries(GAME_DISPLAY).map(([id, entry]) => ({
  id,
  name: entry.name,
  enabled: true,
}));

export const GAME_LABELS: Record<string, string> = Object.fromEntries(
  Object.entries(GAME_DISPLAY).map(([id, entry]) => [id, entry.name])
);

export const CLASS_COLORS: Record<string, string> = {
  A: "bg-rp-red text-white",
  B: "bg-orange-500 text-white",
  C: "bg-amber-500 text-black",
  D: "bg-green-500 text-white",
};

// Game logo SVG paths (kiosk basePath-prefixed, assets in kiosk/public/game-logos/)
export const GAME_LOGO_MAP: Record<string, string> = {
  assetto_corsa: "/kiosk/game-logos/assetto-corsa.svg",
  assetto_corsa_evo: "/kiosk/game-logos/assetto-corsa-evo.svg",
  assetto_corsa_rally: "/kiosk/game-logos/assetto-corsa-rally.svg",
  f1_25: "/kiosk/game-logos/f1-25.svg",
  iracing: "/kiosk/game-logos/iracing.svg",
  le_mans_ultimate: "/kiosk/game-logos/le-mans-ultimate.svg",
  forza_horizon_5: "/kiosk/game-logos/forza-horizon-5.svg",
  ea_wrc: "/kiosk/game-logos/ea-wrc.svg",
};

// Session type icon glyphs for kiosk card grid
export const SESSION_ICONS: Record<string, string> = {
  trackday: "🏁",
  race: "🏆",
  hotlap: "⚡",
  practice: "🔄",
};

export const DIFFICULTY_PRESETS: Record<
  string,
  { label: string; desc: string; aids: Record<string, number> }
> = {
  easy: {
    label: "Easy",
    desc: "ABS, TC, Stability, Ideal Line",
    aids: { abs: 1, tc: 1, stability: 1, autoclutch: 1, ideal_line: 1 },
  },
  medium: {
    label: "Medium",
    desc: "ABS, TC only",
    aids: { abs: 1, tc: 1, stability: 0, autoclutch: 1, ideal_line: 0 },
  },
  hard: {
    label: "Hard",
    desc: "No assists",
    aids: { abs: 0, tc: 0, stability: 0, autoclutch: 0, ideal_line: 0 },
  },
};
