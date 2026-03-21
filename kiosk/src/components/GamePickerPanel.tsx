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

function GameLogoChip({ simType }: { simType: string }) {
  const entry = GAME_DISPLAY[simType];
  const [imgFailed, setImgFailed] = useState(false);

  if (!entry) {
    return (
      <span
        className="w-10 h-10 flex items-center justify-center rounded-md text-xs font-semibold text-white"
        style={{ backgroundColor: "#333333" }}
      >
        ?
      </span>
    );
  }

  if (imgFailed) {
    return (
      <span
        className="w-10 h-10 flex items-center justify-center rounded-md text-xs font-semibold text-white"
        style={{ backgroundColor: "#333333" }}
      >
        {entry.abbr}
      </span>
    );
  }

  return (
    <img
      src={entry.logo}
      alt={entry.name}
      className="w-10 h-10 rounded-md object-contain"
      onError={() => setImgFailed(true)}
    />
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
    <div
      className="flex flex-col gap-4 p-4 rounded-lg border"
      style={{
        backgroundColor: "var(--color-rp-surface, #2A2A2A)",
        borderColor: "var(--color-rp-border, #333333)",
      }}
    >
      <h2
        className="font-semibold"
        style={{ fontSize: "20px", lineHeight: "1.2", fontFamily: "var(--font-sans)" }}
      >
        Select Game for Pod {podNumber}
      </h2>

      {gamesToShow.length === 0 ? (
        <div className="py-6 text-center flex flex-col gap-1">
          <p className="text-white text-sm">No additional games installed on this pod</p>
          <p className="text-sm" style={{ color: "#5A5A5A" }}>
            Only Assetto Corsa is available. Update the pod TOML to add more games.
          </p>
        </div>
      ) : (
        <ul className="flex flex-col gap-2">
          {gamesToShow.map((simType) => {
            const entry = GAME_DISPLAY[simType];
            return (
              <li
                key={simType}
                className="flex items-center gap-3 py-2 border-b"
                style={{ borderColor: "var(--color-rp-border, #333333)" }}
              >
                <GameLogoChip simType={simType} />
                <span className="flex-1 text-white" style={{ fontSize: "14px", fontWeight: 400 }}>
                  {entry.name}
                </span>
                <button
                  onClick={() => onLaunch(podId, simType)}
                  className="px-3 py-1.5 rounded-md text-white transition-colors"
                  style={{
                    fontSize: "12px",
                    fontWeight: 600,
                    backgroundColor: "var(--color-rp-red, #E10600)",
                  }}
                  onMouseEnter={(e) => {
                    (e.currentTarget as HTMLButtonElement).style.backgroundColor =
                      "var(--color-rp-red-hover, #FF1A1A)";
                  }}
                  onMouseLeave={(e) => {
                    (e.currentTarget as HTMLButtonElement).style.backgroundColor =
                      "var(--color-rp-red, #E10600)";
                  }}
                >
                  Launch {entry.name}
                </button>
              </li>
            );
          })}
        </ul>
      )}

      <button
        onClick={onClose}
        className="mt-2 text-white self-start transition-opacity hover:opacity-70"
        style={{ fontSize: "14px" }}
      >
        Cancel
      </button>
    </div>
  );
}
