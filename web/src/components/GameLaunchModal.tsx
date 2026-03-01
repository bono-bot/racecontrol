"use client";

import { useState } from "react";

interface GameLaunchModalProps {
  podId: string;
  podName: string;
  onClose: () => void;
  onLaunch: (simType: string, launchArgs?: string) => void;
}

const GAMES = [
  {
    id: "assetto_corsa",
    name: "Assetto Corsa",
    icon: "AC",
    color: "text-red-400",
    bg: "bg-red-500/10",
    border: "border-red-500",
  },
  {
    id: "iracing",
    name: "iRacing",
    icon: "iR",
    color: "text-blue-400",
    bg: "bg-blue-500/10",
    border: "border-blue-500",
  },
  {
    id: "f1_25",
    name: "F1 25",
    icon: "F1",
    color: "text-red-400",
    bg: "bg-red-500/10",
    border: "border-red-500",
  },
  {
    id: "le_mans_ultimate",
    name: "Le Mans Ultimate",
    icon: "LM",
    color: "text-emerald-400",
    bg: "bg-emerald-500/10",
    border: "border-emerald-500",
  },
  {
    id: "forza",
    name: "Forza Motorsport",
    icon: "FM",
    color: "text-green-400",
    bg: "bg-green-500/10",
    border: "border-green-500",
  },
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

      <div className="relative w-full max-w-lg bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl p-6 mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-lg font-bold text-zinc-100">Launch Game</h2>
            <p className="text-sm text-zinc-500">{podName}</p>
          </div>
          <button
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-300 transition-colors text-xl leading-none"
          >
            &times;
          </button>
        </div>

        <div className="space-y-6">
          {/* Game Selection */}
          <div>
            <label className="block text-sm font-medium text-zinc-300 mb-2">
              Select Game
            </label>
            <div className="grid grid-cols-2 gap-2">
              {GAMES.map((game) => {
                const isSelected = selectedGame === game.id;
                return (
                  <button
                    key={game.id}
                    onClick={() => setSelectedGame(game.id)}
                    className={`rounded-lg border p-3 text-left transition-all ${
                      isSelected
                        ? `${game.border} ${game.bg}`
                        : "border-zinc-700 bg-zinc-800 hover:border-zinc-600"
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <span
                        className={`text-lg font-bold ${
                          isSelected ? game.color : "text-zinc-500"
                        }`}
                      >
                        {game.icon}
                      </span>
                      <span className="text-sm font-medium text-zinc-200">
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
            <label className="block text-sm font-medium text-zinc-300 mb-2">
              Launch Arguments{" "}
              <span className="text-zinc-600 font-normal">(optional)</span>
            </label>
            <input
              type="text"
              placeholder="e.g. -fullscreen -monitor 2"
              value={launchArgs}
              onChange={(e) => setLaunchArgs(e.target.value)}
              className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-orange-500 transition-colors"
            />
          </div>

          {/* Launch Button */}
          <button
            onClick={handleLaunch}
            disabled={!selectedGame || launching}
            className={`w-full rounded-lg py-3 font-semibold text-sm transition-all ${
              selectedGame && !launching
                ? "bg-orange-500 text-white hover:bg-orange-600 active:bg-orange-700"
                : "bg-zinc-800 text-zinc-600 cursor-not-allowed"
            }`}
          >
            {launching ? "Launching..." : "Launch Game"}
          </button>
        </div>
      </div>
    </div>
  );
}
