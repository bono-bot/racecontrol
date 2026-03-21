// Shared game display metadata for all multi-game UI components.
// TODO: Replace placeholder logos with actual game logos
export interface GameDisplayEntry {
  name: string;
  logo: string;
  abbr: string;
}

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
};
