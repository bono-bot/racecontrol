"use client";

import { useState } from "react";
import type { KioskExperience } from "@/lib/types";
import { GAME_LABELS, CLASS_COLORS } from "@/lib/constants";

// ─── Mock Data ────────────────────────────────────────────────────────────

const mockExperiences: KioskExperience[] = [
  { id: "1", name: "Monza GT3 Trackday", game: "assetto_corsa", track: "Autodromo di Monza", car: "Ferrari 488 GT3 Evo", car_class: "GT3", duration_minutes: 30, start_type: "trackday", sort_order: 1, is_active: true },
  { id: "2", name: "Spa Endurance", game: "assetto_corsa", track: "Spa-Francorchamps", car: "Porsche 911 RSR", car_class: "GTE", duration_minutes: 60, start_type: "race", sort_order: 2, is_active: true },
  { id: "3", name: "Nürburgring Hotlap", game: "assetto_corsa", track: "Nürburgring Nordschleife", car: "Lamborghini Huracán ST", car_class: "GT3", duration_minutes: 30, start_type: "hotlap", sort_order: 3, is_active: true },
  { id: "4", name: "Silverstone Open Practice", game: "assetto_corsa", track: "Silverstone", car: "BMW M4 GT3", car_class: "GT3", duration_minutes: 30, start_type: "practice", sort_order: 4, is_active: true },
  { id: "5", name: "Monaco Grand Prix", game: "f1_25", track: "Monaco", car: "McLaren MCL60", car_class: "F1", duration_minutes: 30, start_type: "race", sort_order: 5, is_active: true },
  { id: "6", name: "Bahrain Sprint", game: "f1_25", track: "Sakhir", car: "Red Bull RB20", car_class: "F1", duration_minutes: 30, start_type: "race", sort_order: 6, is_active: true },
  { id: "7", name: "Daytona 24h Practice", game: "le_mans_ultimate", track: "Daytona", car: "Cadillac V-Series.R", car_class: "LMDh", duration_minutes: 60, start_type: "practice", sort_order: 7, is_active: true },
  { id: "8", name: "Imola Sprint Race", game: "assetto_corsa", track: "Autodromo Enzo e Dino Ferrari", car: "Mercedes AMG GT3", car_class: "GT3", duration_minutes: 30, start_type: "race", sort_order: 8, is_active: true },
];

const GAME_LOGO_MAP: Record<string, string> = {
  assetto_corsa: "/kiosk/game-logos/assetto-corsa.svg",
  f1_25: "/kiosk/game-logos/f1-25.svg",
  iracing: "/kiosk/game-logos/iracing.svg",
  le_mans_ultimate: "/kiosk/game-logos/le-mans-ultimate.svg",
  assetto_corsa_evo: "/kiosk/game-logos/assetto-corsa-evo.svg",
};

const SESSION_ICONS: Record<string, string> = {
  trackday: "🏁",
  race: "🏆",
  hotlap: "⚡",
  practice: "🔄",
};

// ─── Game Tab ─────────────────────────────────────────────────────────────

function GameTab({ game, active, onClick }: { game: string; active: boolean; onClick: () => void }) {
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

function ExperienceCard({
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
        <div className="absolute inset-0 opacity-5" style={{
          backgroundImage: "repeating-linear-gradient(45deg, transparent, transparent 10px, rgba(255,255,255,0.03) 10px, rgba(255,255,255,0.03) 20px)"
        }} />

        {/* Game logo watermark */}
        {logo && (
          <img src={logo} alt="" className="h-12 w-auto object-contain opacity-20 group-hover:opacity-30 transition-opacity" />
        )}

        {/* Car class badge — top right */}
        {exp.car_class && (
          <span className={`absolute top-3 right-3 px-2.5 py-1 rounded-md text-xs font-bold uppercase tracking-wider ${
            CLASS_COLORS[exp.car_class] || "bg-zinc-700 text-white"
          }`}>
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
        <p className="text-sm text-zinc-400 truncate">
          {exp.track}
        </p>
        <p className="text-sm text-zinc-500 font-mono truncate">
          {exp.car}
        </p>

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
            <span className="text-xs text-zinc-600 italic">
              Not installed
            </span>
          )}
        </div>
      </div>
    </button>
  );
}

// ─── Page ─────────────────────────────────────────────────────────────────

export default function IdlePreview() {
  const [gameFilter, setGameFilter] = useState<string>("all");

  const gameTabs = ["all", ...new Set(mockExperiences.map((e) => e.game))];
  const filtered = gameFilter === "all"
    ? mockExperiences
    : mockExperiences.filter((e) => e.game === gameFilter);

  return (
    <div className="h-screen w-screen bg-rp-black flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-8 py-5">
        <div className="flex items-center gap-4">
          <div className="w-1 h-8 bg-rp-red rounded-full" />
          <div>
            <h1 className="text-2xl font-bold text-white tracking-wide" style={{ fontFamily: "var(--font-display, 'Orbitron'), sans-serif" }}>
              Select Experience
            </h1>
            <p className="text-sm text-zinc-500">Choose your race, track, and car</p>
          </div>
        </div>

        {/* Racing Point branding */}
        <div className="text-right">
          <p className="text-xs text-zinc-600 font-mono uppercase tracking-[0.3em]">Racing Point</p>
          <p className="text-xs text-zinc-700 font-mono">eSports &amp; Cafe</p>
        </div>
      </div>

      {/* Red accent line */}
      <div className="h-[2px] bg-gradient-to-r from-rp-red/60 via-rp-red/20 to-transparent mx-8" />

      {/* Game filter tabs */}
      <div className="flex gap-2 px-8 py-4">
        {gameTabs.map((game) => (
          <GameTab
            key={game}
            game={game}
            active={gameFilter === game}
            onClick={() => setGameFilter(game)}
          />
        ))}
      </div>

      {/* Experience cards grid */}
      <div className="flex-1 overflow-y-auto px-8 pb-8">
        <div className="grid grid-cols-4 gap-4">
          {filtered.map((exp) => (
            <ExperienceCard
              key={exp.id}
              exp={exp}
              available={true}
              onSelect={() => alert(`Selected: ${exp.name}`)}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
