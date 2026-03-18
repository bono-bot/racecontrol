// ─── Shared Constants ────────────────────────────────────────────────────────
// Single source of truth for game-related constants used across kiosk pages.

export const GAMES = [
  { id: "assetto_corsa", name: "Assetto Corsa", enabled: true },
  { id: "assetto_corsa_evo", name: "AC EVO", enabled: true },
  { id: "assetto_corsa_rally", name: "AC Rally", enabled: true },
  { id: "f1_25", name: "F1 25", enabled: true },
  { id: "iracing", name: "iRacing", enabled: true },
  { id: "le_mans_ultimate", name: "Le Mans Ultimate", enabled: true },
  { id: "forza", name: "Forza Motorsport", enabled: false },
  { id: "forza_horizon_5", name: "Forza Horizon 5", enabled: true },
] as const;

export const GAME_LABELS: Record<string, string> = {
  assetto_corsa: "Assetto Corsa",
  assetto_corsa_evo: "AC EVO",
  assetto_corsa_rally: "AC Rally",
  f1_25: "F1 25",
  iracing: "iRacing",
  le_mans_ultimate: "Le Mans Ultimate",
  forza: "Forza Motorsport",
  forza_horizon_5: "Forza Horizon 5",
};

export const CLASS_COLORS: Record<string, string> = {
  A: "bg-rp-red text-white",
  B: "bg-orange-500 text-white",
  C: "bg-amber-500 text-black",
  D: "bg-green-500 text-white",
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
