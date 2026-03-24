// Shared game display metadata for all multi-game UI components.
// Fallback data — used when /api/v1/games/catalog is unreachable.
// The API is the authoritative source; this is the offline safety net.

export interface GameDisplayEntry {
  name: string;
  logo: string;
  abbr: string;
}

// Fallback catalog — kept in sync with SimType enum in rc-common/types.rs
export const GAME_DISPLAY: Record<string, GameDisplayEntry> = {
  assetto_corsa: {
    name: "Assetto Corsa",
    logo: "/game-logos/assetto-corsa.png",
    abbr: "AC",
  },
  assetto_corsa_evo: {
    name: "AC EVO",
    logo: "/game-logos/assetto-corsa-evo.png",
    abbr: "ACE",
  },
  assetto_corsa_rally: {
    name: "EA WRC",
    logo: "/game-logos/assetto-corsa-rally.png",
    abbr: "ACR",
  },
  iracing: {
    name: "iRacing",
    logo: "/game-logos/iracing.png",
    abbr: "iR",
  },
  f1_25: {
    name: "F1 25",
    logo: "/game-logos/f1-25.png",
    abbr: "F1",
  },
  le_mans_ultimate: {
    name: "Le Mans Ultimate",
    logo: "/game-logos/le-mans-ultimate.png",
    abbr: "LMU",
  },
  forza: {
    name: "Forza Motorsport",
    logo: "/game-logos/forza.png",
    abbr: "FRZ",
  },
  forza_horizon_5: {
    name: "Forza Horizon 5",
    logo: "/game-logos/forza-horizon-5.png",
    abbr: "FH5",
  },
};

// Merge API catalog data into GAME_DISPLAY at runtime.
// Called once on app load — enriches the fallback with any new games from the backend.
export function mergeApiCatalog(
  apiGames: { id: string; name: string; abbr: string }[]
): Record<string, GameDisplayEntry> {
  const merged = { ...GAME_DISPLAY };
  for (const game of apiGames) {
    if (!merged[game.id]) {
      // New game from API that isn't in fallback — add it dynamically
      merged[game.id] = {
        name: game.name,
        logo: `/game-logos/${game.id.replace(/_/g, "-")}.png`,
        abbr: game.abbr,
      };
    } else {
      // Update name/abbr from API (API is authoritative)
      merged[game.id] = {
        ...merged[game.id],
        name: game.name,
        abbr: game.abbr,
      };
    }
  }
  return merged;
}
