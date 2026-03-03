"use client";

import { useState, useEffect } from "react";
import { api } from "@/lib/api";
import type { KioskExperience } from "@/lib/types";

interface ExperienceSelectorProps {
  podId: string;
  onSelect: (experience: KioskExperience) => void;
  onCancel: () => void;
}

const GAME_LABELS: Record<string, string> = {
  assetto_corsa: "Assetto Corsa",
  f1_25: "F1 25",
  assetto_corsa_rally: "AC Rally",
  le_mans_ultimate: "LeMans Ultimate",
  iracing: "iRacing",
};

const CLASS_COLORS: Record<string, string> = {
  A: "bg-rp-red text-white",
  B: "bg-orange-500 text-white",
  C: "bg-amber-500 text-black",
  D: "bg-green-500 text-white",
};

export function ExperienceSelector({ podId, onSelect, onCancel }: ExperienceSelectorProps) {
  const [experiences, setExperiences] = useState<KioskExperience[]>([]);
  const [selectedGame, setSelectedGame] = useState<string>("all");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.listExperiences().then((res) => {
      setExperiences(res.experiences || []);
      setLoading(false);
    });
  }, []);

  const games = ["all", ...new Set(experiences.map((e) => e.game))];
  const filtered =
    selectedGame === "all"
      ? experiences
      : experiences.filter((e) => e.game === selectedGame);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-rp-card border border-rp-border rounded-lg w-full max-w-lg shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3 border-b border-rp-border">
          <h2 className="text-sm font-semibold">Select Experience — Pod {podId.slice(-4)}</h2>
          <button onClick={onCancel} className="text-rp-grey hover:text-white text-lg">&times;</button>
        </div>

        <div className="p-5 space-y-4">
          {/* Game Tabs */}
          <div className="flex gap-2 flex-wrap">
            {games.map((game) => (
              <button
                key={game}
                onClick={() => setSelectedGame(game)}
                className={`px-3 py-1.5 text-xs rounded-full border transition-colors ${
                  selectedGame === game
                    ? "border-rp-red text-rp-red bg-rp-red/10"
                    : "border-rp-border text-rp-grey hover:text-white"
                }`}
              >
                {game === "all" ? "All" : GAME_LABELS[game] || game}
              </button>
            ))}
          </div>

          {/* Experience List */}
          {loading ? (
            <div className="flex justify-center py-8">
              <div className="w-6 h-6 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
            </div>
          ) : filtered.length === 0 ? (
            <p className="text-sm text-rp-grey text-center py-8">
              No experiences configured
            </p>
          ) : (
            <div className="space-y-2 max-h-80 overflow-y-auto">
              {filtered.map((exp) => (
                <button
                  key={exp.id}
                  onClick={() => onSelect(exp)}
                  className="w-full flex items-center gap-3 px-4 py-3 bg-rp-surface border border-rp-border rounded hover:border-rp-red/50 transition-colors text-left"
                >
                  {/* Class Badge */}
                  {exp.car_class && (
                    <span
                      className={`w-7 h-7 flex items-center justify-center rounded text-xs font-bold ${
                        CLASS_COLORS[exp.car_class] || "bg-zinc-600 text-white"
                      }`}
                    >
                      {exp.car_class}
                    </span>
                  )}
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-semibold text-white truncate">{exp.name}</p>
                    <p className="text-xs text-rp-grey truncate">
                      {exp.track} &middot; {exp.car}
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="text-xs text-rp-grey">{exp.duration_minutes}min</p>
                    <p className="text-[10px] text-rp-grey capitalize">{exp.start_type}</p>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
