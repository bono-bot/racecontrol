"use client";

import { useState } from "react";
import { GAME_DISPLAY } from "@/lib/gameDisplayInfo";

interface GamePickerPanelProps {
  podId: string;
  podNumber: number;
  installedGames: string[]; // SimType string values from pod.installed_games
  onLaunch: (podId: string, simType: string) => void;
  onClose: () => void;
}

function GameLogoCard({ simType, onLaunch }: { simType: string; onLaunch: () => void }) {
  const entry = GAME_DISPLAY[simType];
  const [imgFailed, setImgFailed] = useState(false);

  if (!entry) return null;

  return (
    <button
      onClick={onLaunch}
      className="flex flex-col items-center gap-2.5 p-4 rounded-lg border border-[#2A2A2A] bg-[#141414] hover:bg-[#1E1E1E] hover:border-rp-red/40 cursor-pointer transition-all duration-200 group min-h-[120px] justify-center"
    >
      {!imgFailed ? (
        <img
          src={entry.logo}
          alt={entry.name}
          className="w-14 h-14 rounded-lg object-contain group-hover:scale-105 transition-transform duration-200"
          onError={() => setImgFailed(true)}
        />
      ) : (
        <span className="w-14 h-14 flex items-center justify-center rounded-lg bg-[#1E1E1E] text-sm font-bold text-white font-mono">
          {entry.abbr}
        </span>
      )}
      <span className="text-xs text-zinc-400 font-medium text-center group-hover:text-white transition-colors duration-200">
        {entry.name}
      </span>
    </button>
  );
}

export function GamePickerPanel({
  podId,
  podNumber,
  installedGames,
  onLaunch,
  onClose,
}: GamePickerPanelProps) {
  // Show all installed games (including AC — clicking AC triggers wizard via parent onLaunch)
  const gamesToShow = installedGames.filter((g) => GAME_DISPLAY[g] !== undefined);

  return (
    <div className="flex flex-col gap-5 p-5 h-full">
      {gamesToShow.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center gap-2">
          <p className="text-white text-sm font-sans">No additional games installed on this pod</p>
          <p className="text-xs text-zinc-600">
            Only Assetto Corsa is available. Update the pod TOML to add more games.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-3 gap-3">
          {gamesToShow.map((simType) => (
            <GameLogoCard
              key={simType}
              simType={simType}
              onLaunch={() => onLaunch(podId, simType)}
            />
          ))}
        </div>
      )}

      <button
        onClick={onClose}
        className="mt-auto w-full py-2.5 border border-[#2A2A2A] text-zinc-500 hover:text-white rounded-lg text-sm cursor-pointer transition-colors duration-200"
      >
        Cancel
      </button>
    </div>
  );
}
