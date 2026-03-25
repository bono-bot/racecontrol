"use client";

import { useState, useEffect } from "react";
import { fetchPublic } from "@/lib/api";

interface GameLaunchModalProps {
  podId: string;
  podName: string;
  onClose: () => void;
  onLaunch: (simType: string, launchArgs?: string) => void;
}

interface GameEntry {
  id: string;
  name: string;
  icon: string;
  color: string;
  bg: string;
  border: string;
}

// Display metadata per sim_type — keyed by the snake_case id from /games/catalog
const GAME_STYLES: Record<string, { icon: string; color: string; bg: string; border: string }> = {
  assetto_corsa:       { icon: "AC",  color: "text-red-400",     bg: "bg-red-500/10",     border: "border-red-500" },
  assetto_corsa_evo:   { icon: "ACE", color: "text-teal-400",    bg: "bg-teal-500/10",    border: "border-teal-500" },
  assetto_corsa_rally: { icon: "ACR", color: "text-orange-400",  bg: "bg-orange-500/10",  border: "border-orange-500" },
  iracing:             { icon: "iR",  color: "text-blue-400",    bg: "bg-blue-500/10",    border: "border-blue-500" },
  f1_25:               { icon: "F1",  color: "text-red-400",     bg: "bg-red-500/10",     border: "border-red-500" },
  le_mans_ultimate:    { icon: "LM",  color: "text-emerald-400", bg: "bg-emerald-500/10", border: "border-emerald-500" },
  forza:               { icon: "FM",  color: "text-green-400",   bg: "bg-green-500/10",   border: "border-green-500" },
  forza_horizon_5:     { icon: "FH5", color: "text-yellow-400",  bg: "bg-yellow-500/10",  border: "border-yellow-500" },
};

// Fallback when /games/catalog is unreachable — kept in sync with SimType enum in rc-common/types.rs
const FALLBACK_GAMES: GameEntry[] = [
  { id: "assetto_corsa",       name: "Assetto Corsa",     ...GAME_STYLES.assetto_corsa },
  { id: "assetto_corsa_evo",   name: "AC EVO",            ...GAME_STYLES.assetto_corsa_evo },
  { id: "assetto_corsa_rally", name: "EA WRC",            ...GAME_STYLES.assetto_corsa_rally },
  { id: "iracing",             name: "iRacing",           ...GAME_STYLES.iracing },
  { id: "f1_25",               name: "F1 25",             ...GAME_STYLES.f1_25 },
  { id: "le_mans_ultimate",    name: "Le Mans Ultimate",  ...GAME_STYLES.le_mans_ultimate },
  { id: "forza",               name: "Forza Motorsport",  ...GAME_STYLES.forza },
  { id: "forza_horizon_5",     name: "Forza Horizon 5",   ...GAME_STYLES.forza_horizon_5 },
];

export default function GameLaunchModal({
  podId,
  podName,
  onClose,
  onLaunch,
}: GameLaunchModalProps) {
  const [selectedGame, setSelectedGame] = useState<string | null>(null);
  const [launchArgs, setLaunchArgs] = useState("");
  const [launching, setLaunching] = useState(false);
  const [games, setGames] = useState<GameEntry[]>(FALLBACK_GAMES);

  // Fetch authoritative game list from /games/catalog, fall back to hardcoded
  useEffect(() => {
    fetchPublic<{ games: Array<{ id: string; name: string; abbr: string }> }>("/games/catalog")
      .then((data) => {
        if (data?.games?.length) {
          const mapped: GameEntry[] = data.games.map((g) => {
            const style = GAME_STYLES[g.id] ?? {
              icon: g.abbr || g.id.slice(0, 3).toUpperCase(),
              color: "text-neutral-400",
              bg: "bg-neutral-500/10",
              border: "border-neutral-500",
            };
            return { id: g.id, name: g.name, ...style };
          });
          setGames(mapped);
        }
      })
      .catch(() => {
        // Keep fallback — already set
      });
  }, []);

  function handleLaunch() {
    if (!selectedGame) return;
    setLaunching(true);
    onLaunch(selectedGame, launchArgs || undefined);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-sm"
        onClick={onClose}
      />

      <div className="relative w-full max-w-lg bg-rp-card border border-rp-border rounded-xl shadow-2xl p-6 mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-lg font-bold text-white">Launch Game</h2>
            <p className="text-sm text-rp-grey">{podName}</p>
          </div>
          <button
            onClick={onClose}
            className="text-rp-grey hover:text-neutral-300 transition-colors text-xl leading-none"
          >
            &times;
          </button>
        </div>

        <div className="space-y-6">
          {/* Game Selection */}
          <div>
            <label className="block text-sm font-medium text-neutral-300 mb-2">
              Select Game
            </label>
            <div className="grid grid-cols-2 gap-2">
              {games.map((game) => {
                const isSelected = selectedGame === game.id;
                return (
                  <button
                    key={game.id}
                    onClick={() => setSelectedGame(game.id)}
                    className={`rounded-lg border p-3 text-left transition-all ${
                      isSelected
                        ? `${game.border} ${game.bg}`
                        : "border-rp-border bg-rp-card hover:border-rp-border"
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <span
                        className={`text-lg font-bold ${
                          isSelected ? game.color : "text-rp-grey"
                        }`}
                      >
                        {game.icon}
                      </span>
                      <span className="text-sm font-medium text-neutral-200">
                        {game.name}
                      </span>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Launch Args (optional) */}
          <div>
            <label className="block text-sm font-medium text-neutral-300 mb-2">
              Launch Arguments{" "}
              <span className="text-rp-grey font-normal">(optional)</span>
            </label>
            <input
              type="text"
              placeholder="e.g. -fullscreen -monitor 2"
              value={launchArgs}
              onChange={(e) => setLaunchArgs(e.target.value)}
              className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-rp-grey focus:outline-none focus:border-rp-red transition-colors"
            />
          </div>

          {/* Launch Button */}
          <button
            onClick={handleLaunch}
            disabled={!selectedGame || launching}
            className={`w-full rounded-lg py-3 font-semibold text-sm transition-all ${
              selectedGame && !launching
                ? "bg-rp-red text-white hover:bg-rp-red active:bg-rp-red"
                : "bg-rp-card text-rp-grey cursor-not-allowed"
            }`}
          >
            {launching ? "Launching..." : "Launch Game"}
          </button>
        </div>
      </div>
    </div>
  );
}
