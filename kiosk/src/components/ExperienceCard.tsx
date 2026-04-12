"use client";

import type { KioskExperience } from "@/lib/types";
import { GAME_LABELS, CLASS_COLORS, GAME_LOGO_MAP, SESSION_ICONS } from "@/lib/constants";

// ─── Game Tab ─────────────────────────────────────────────────────────────

export function GameTab({
  game,
  active,
  onClick,
}: {
  game: string;
  active: boolean;
  onClick: () => void;
}) {
  const logo = game !== "all" ? GAME_LOGO_MAP[game] : null;
  const label = game === "all" ? "All Games" : GAME_LABELS[game] || game;

  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-2 px-5 py-2.5 rounded-lg border transition-all ${
        active
          ? "border-rp-red bg-rp-red/10 text-white"
          : "border-zinc-800 bg-zinc-900/50 text-zinc-400 hover:text-white hover:border-zinc-700"
      }`}
    >
      {logo && (
        <img src={logo} alt="" className="h-5 w-auto object-contain opacity-80" />
      )}
      <span className="text-sm font-semibold tracking-wide">{label}</span>
    </button>
  );
}

// ─── Experience Card ──────────────────────────────────────────────────────

export function ExperienceCard({
  exp,
  available,
  onSelect,
}: {
  exp: KioskExperience;
  available: boolean;
  onSelect: () => void;
}) {
  const logo = GAME_LOGO_MAP[exp.game];
  const icon = SESSION_ICONS[exp.start_type] || "🏁";

  return (
    <button
      onClick={() => available && onSelect()}
      disabled={!available}
      className={`group relative flex flex-col rounded-xl border overflow-hidden transition-all duration-200 text-left ${
        available
          ? "border-zinc-800 bg-zinc-900/60 hover:border-rp-red/50 hover:bg-zinc-900/80 cursor-pointer hover:scale-[1.02] hover:shadow-[0_0_30px_rgba(225,6,0,0.1)]"
          : "border-zinc-800/30 bg-zinc-900/20 cursor-not-allowed opacity-40"
      }`}
    >
      {/* Top section — gradient background with game logo */}
      <div className="relative h-28 bg-gradient-to-br from-zinc-800/80 via-zinc-900 to-zinc-950 flex items-center justify-center overflow-hidden">
        {/* Subtle pattern overlay */}
        <div
          className="absolute inset-0 opacity-5"
          style={{
            backgroundImage:
              "repeating-linear-gradient(45deg, transparent, transparent 10px, rgba(255,255,255,0.03) 10px, rgba(255,255,255,0.03) 20px)",
          }}
        />

        {/* Game logo watermark */}
        {logo && (
          <img
            src={logo}
            alt=""
            className="h-12 w-auto object-contain opacity-20 group-hover:opacity-30 transition-opacity"
          />
        )}

        {/* Car class badge — top right */}
        {exp.car_class && (
          <span
            className={`absolute top-3 right-3 px-2.5 py-1 rounded-md text-xs font-bold uppercase tracking-wider ${
              CLASS_COLORS[exp.car_class] || "bg-zinc-700 text-white"
            }`}
          >
            {exp.car_class}
          </span>
        )}

        {/* Session type badge — top left */}
        <span className="absolute top-3 left-3 flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-zinc-950/70 text-xs text-zinc-300 font-mono uppercase tracking-wider">
          <span>{icon}</span>
          {exp.start_type}
        </span>

        {/* Red accent line at bottom of image area */}
        <div className="absolute bottom-0 left-0 right-0 h-[2px] bg-gradient-to-r from-rp-red/50 via-rp-red/20 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
      </div>

      {/* Content section */}
      <div className="flex-1 flex flex-col p-4 gap-1.5">
        {/* Experience name */}
        <h3 className="text-lg font-bold text-white truncate group-hover:text-rp-red-hover transition-colors">
          {exp.name}
        </h3>

        {/* Track + Car */}
        <p className="text-sm text-zinc-400 truncate">{exp.track}</p>
        <p className="text-sm text-zinc-500 font-mono truncate">{exp.car}</p>

        {/* Bottom row — duration + availability */}
        <div className="mt-auto pt-2 flex items-center justify-between">
          <span className="text-xs text-zinc-500 font-mono">
            {exp.duration_minutes} min
          </span>
          {available ? (
            <span className="text-xs text-rp-green font-semibold uppercase tracking-wider">
              Available
            </span>
          ) : (
            <span className="text-xs text-zinc-600 italic">Not installed</span>
          )}
        </div>
      </div>
    </button>
  );
}
