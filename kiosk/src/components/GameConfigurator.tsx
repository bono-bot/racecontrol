"use client";

import { useState, useEffect, useMemo } from "react";
import { api } from "@/lib/api";
import type { AcCatalog, CatalogItem, PresetEntry } from "@/lib/types";

interface GameConfiguratorProps {
  podId: string;
  podNumber: number;
  driverName: string;
  onLaunch: (simType: string, launchArgs: string) => void;
  onCancel: () => void;
}

type ConfigStep = "presets" | "game" | "mode" | "track" | "car" | "settings" | "review";

const DIFFICULTY_PRESETS: Record<string, { label: string; desc: string; aids: Record<string, number> }> = {
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

const GAMES = [
  { id: "assetto_corsa", name: "Assetto Corsa", enabled: true },
  { id: "assetto_corsa_evo", name: "Assetto Corsa Evo", enabled: true },
  { id: "f1_25", name: "F1 25", enabled: true },
  { id: "iracing", name: "iRacing", enabled: true },
  { id: "le_mans_ultimate", name: "Le Mans Ultimate", enabled: true },
  { id: "forza", name: "Forza Motorsport", enabled: false },
];

export function GameConfigurator({ podId, podNumber, driverName, onLaunch, onCancel }: GameConfiguratorProps) {
  const [step, setStep] = useState<ConfigStep>("presets");
  const [catalog, setCatalog] = useState<AcCatalog | null>(null);
  const [loading, setLoading] = useState(true);

  // Selections
  const [game, setGame] = useState("");
  const [gameMode, setGameMode] = useState("single");
  const [track, setTrack] = useState<CatalogItem | null>(null);
  const [car, setCar] = useState<CatalogItem | null>(null);
  const [difficulty, setDifficulty] = useState("easy");
  const [transmission, setTransmission] = useState("auto");
  const [ffb, setFfb] = useState("medium");

  // Search/filter
  const [trackSearch, setTrackSearch] = useState("");
  const [trackCategory, setTrackCategory] = useState("Featured");
  const [carSearch, setCarSearch] = useState("");
  const [carCategory, setCarCategory] = useState("Featured");

  useEffect(() => {
    api.getAcCatalog().then((data) => {
      setCatalog(data);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  // Filtered tracks
  const filteredTracks = useMemo(() => {
    if (!catalog) return [];
    let items = trackCategory === "Featured" ? catalog.tracks.featured : catalog.tracks.all;
    if (trackCategory !== "Featured" && trackCategory !== "All") {
      items = catalog.tracks.all.filter((t) => t.category === trackCategory);
    }
    if (trackSearch) {
      const q = trackSearch.toLowerCase();
      items = items.filter((t) => t.name.toLowerCase().includes(q) || (t.country || "").toLowerCase().includes(q));
    }
    return items;
  }, [catalog, trackCategory, trackSearch]);

  // Filtered cars
  const filteredCars = useMemo(() => {
    if (!catalog) return [];
    let items = carCategory === "Featured" ? catalog.cars.featured : catalog.cars.all;
    if (carCategory !== "Featured" && carCategory !== "All") {
      items = catalog.cars.all.filter((c) => c.category === carCategory);
    }
    if (carSearch) {
      const q = carSearch.toLowerCase();
      items = items.filter((c) => c.name.toLowerCase().includes(q));
    }
    return items;
  }, [catalog, carCategory, carSearch]);

  const trackCategories = ["Featured", ...(catalog?.categories.tracks || []), "All"];
  const carCategories = ["Featured", ...(catalog?.categories.cars || []), "All"];

  // Preset quick-pick: pre-fill selections and jump to review
  function selectPreset(preset: PresetEntry) {
    if (!catalog) return;
    const foundTrack = catalog.tracks.all.find((t) => t.id === preset.track_id) || null;
    const foundCar = catalog.cars.all.find((c) => c.id === preset.car_id) || null;
    if (!foundTrack || !foundCar) return;
    setGame("assetto_corsa");
    setTrack(foundTrack);
    setCar(foundCar);
    setDifficulty(["easy", "medium", "hard"].includes(preset.difficulty) ? preset.difficulty : "easy");
    setStep("review");
  }

  function handleLaunch() {
    const preset = DIFFICULTY_PRESETS[difficulty];
    const launchArgs = JSON.stringify({
      car: car?.id || "",
      track: track?.id || "",
      driver: driverName,
      difficulty,
      transmission,
      ffb,
      game,
      game_mode: gameMode,
      aids: preset?.aids || { abs: 1, tc: 1, stability: 1, autoclutch: 1, ideal_line: 1 },
      conditions: { damage: 0 },
    });
    onLaunch(game, launchArgs);
  }

  // Step navigation
  function goBack() {
    const steps: ConfigStep[] = ["presets", "game", "mode", "track", "car", "settings", "review"];
    const idx = steps.indexOf(step);
    if (idx <= 0) onCancel();
    else setStep(steps[idx - 1]);
  }

  if (loading) {
    return (
      <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center">
        <div className="w-12 h-12 border-4 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-rp-card border border-rp-border rounded-2xl w-full max-w-3xl max-h-[90vh] flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-rp-border">
          <div>
            <h2 className="text-xl font-bold text-white">
              {step === "presets" && "Quick Start"}
              {step === "game" && "Select Game"}
              {step === "mode" && "Game Mode"}
              {step === "track" && "Select Track"}
              {step === "car" && "Select Car"}
              {step === "settings" && "Difficulty, Transmission & FFB"}
              {step === "review" && "Review & Launch"}
            </h2>
            <p className="text-sm text-rp-grey">Pod {podNumber} &middot; {driverName}</p>
          </div>
          <button onClick={onCancel} className="text-rp-grey hover:text-white transition-colors">
            <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6">
          {/* ─── PRESET QUICK-PICK ───────────────────────────────── */}
          {step === "presets" && (() => {
            const presets = catalog?.presets || [];
            const featured = presets.filter((p) => p.featured);
            const remaining = presets.filter((p) => !p.featured);

            if (presets.length === 0) {
              return (
                <div className="text-center py-12 space-y-4">
                  <p className="text-rp-grey">No presets available</p>
                  <button
                    onClick={() => { setGame(""); setStep("game"); }}
                    className="px-8 py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-xl transition-colors"
                  >
                    Custom Setup
                  </button>
                </div>
              );
            }

            return (
              <div className="space-y-6">
                {/* Staff Picks */}
                {featured.length > 0 && (
                  <div>
                    <p className="text-xs font-semibold text-rp-grey uppercase tracking-wide mb-3">Staff Picks</p>
                    <div className="grid grid-cols-2 gap-3">
                      {featured.map((preset) => (
                        <button
                          key={preset.id}
                          onClick={() => selectPreset(preset)}
                          className="relative p-5 rounded-xl border-2 border-rp-border bg-rp-surface text-left hover:border-rp-red transition-all group"
                        >
                          <div className={`absolute left-0 top-0 bottom-0 w-1 rounded-l-xl ${
                            preset.category === "Race" ? "bg-[#E10600]" :
                            preset.category === "Casual" ? "bg-[#1a3a5c]" :
                            "bg-[#4a0e4e]"
                          }`} />
                          <span className="absolute top-3 right-3 bg-rp-surface border border-rp-border text-rp-grey text-[10px] font-semibold px-2 py-0.5 rounded-full">
                            {preset.duration_hint}
                          </span>
                          <p className="text-lg font-bold text-white pr-16">{preset.track_name}</p>
                          <p className="text-sm text-rp-grey">{preset.car_name}</p>
                          <p className="text-xs text-rp-grey/70 mt-1 line-clamp-1">{preset.tagline}</p>
                        </button>
                      ))}
                    </div>
                  </div>
                )}

                {/* All Presets */}
                {remaining.length > 0 && (
                  <div>
                    <p className="text-xs font-semibold text-rp-grey uppercase tracking-wide mb-3">All Presets</p>
                    <div className="grid grid-cols-3 gap-2 max-h-[40vh] overflow-y-auto">
                      {remaining.map((preset) => (
                        <button
                          key={preset.id}
                          onClick={() => selectPreset(preset)}
                          className="relative p-4 rounded-xl border border-rp-border bg-rp-surface text-left hover:border-rp-red transition-all"
                        >
                          <div className={`absolute left-0 top-0 bottom-0 w-1 rounded-l-xl ${
                            preset.category === "Race" ? "bg-[#E10600]" :
                            preset.category === "Casual" ? "bg-[#1a3a5c]" :
                            "bg-[#4a0e4e]"
                          }`} />
                          <span className="absolute top-2 right-2 text-rp-grey text-[9px] font-semibold">
                            {preset.duration_hint}
                          </span>
                          <p className="text-sm font-bold text-white pr-10">{preset.track_name}</p>
                          <p className="text-xs text-rp-grey">{preset.car_name}</p>
                          <p className="text-[10px] text-rp-grey/60 mt-0.5 line-clamp-1">{preset.tagline}</p>
                        </button>
                      ))}
                    </div>
                  </div>
                )}

                {/* Custom Setup button */}
                <button
                  onClick={() => { setGame(""); setStep("game"); }}
                  className="w-full p-5 rounded-xl border-2 border-rp-border bg-rp-surface text-center hover:border-rp-red transition-all"
                >
                  <p className="text-lg font-bold text-white">Custom Setup</p>
                  <p className="text-sm text-rp-grey mt-1">Choose your own car, track, and settings</p>
                </button>
              </div>
            );
          })()}

          {/* ─── GAME SELECT ─────────────────────────────────────── */}
          {step === "game" && (
            <div className="grid grid-cols-3 gap-4">
              {GAMES.map((g) => (
                <button
                  key={g.id}
                  disabled={!g.enabled}
                  onClick={() => { setGame(g.id); setStep("mode"); }}
                  className={`p-6 rounded-xl border-2 text-center transition-all ${
                    g.enabled
                      ? "border-rp-border bg-rp-surface hover:border-rp-red hover:bg-rp-red/10 cursor-pointer"
                      : "border-rp-border/50 bg-rp-surface/50 opacity-40 cursor-not-allowed"
                  }`}
                >
                  <p className="text-lg font-bold text-white">{g.name}</p>
                  {!g.enabled && <p className="text-xs text-rp-grey mt-1">Coming Soon</p>}
                </button>
              ))}
            </div>
          )}

          {/* ─── GAME MODE ───────────────────────────────────────── */}
          {step === "mode" && (
            <div className="grid grid-cols-2 gap-6">
              <button
                onClick={() => { setGameMode("single"); setStep("track"); }}
                className="p-8 rounded-xl border-2 border-rp-border bg-rp-surface hover:border-rp-red hover:bg-rp-red/10 transition-all text-center"
              >
                <svg className="w-12 h-12 mx-auto mb-3 text-rp-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                </svg>
                <p className="text-xl font-bold text-white">Singleplayer</p>
                <p className="text-sm text-rp-grey mt-1">Practice &amp; hot laps</p>
              </button>
              <button
                disabled
                className="p-8 rounded-xl border-2 border-rp-border/50 bg-rp-surface/50 opacity-40 cursor-not-allowed text-center"
              >
                <svg className="w-12 h-12 mx-auto mb-3 text-rp-grey" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z" />
                </svg>
                <p className="text-xl font-bold text-white">Multiplayer</p>
                <p className="text-sm text-rp-grey mt-1">Coming Soon</p>
              </button>
            </div>
          )}

          {/* ─── TRACK SELECT ────────────────────────────────────── */}
          {step === "track" && (
            <div className="space-y-4">
              {/* Search */}
              <input
                type="text"
                value={trackSearch}
                onChange={(e) => setTrackSearch(e.target.value)}
                placeholder="Search tracks..."
                className="w-full px-4 py-3 bg-rp-surface border border-rp-border rounded-lg text-white placeholder:text-rp-grey focus:outline-none focus:border-rp-red"
              />
              {/* Category tabs */}
              <div className="flex gap-2 overflow-x-auto pb-2">
                {trackCategories.map((cat) => (
                  <button
                    key={cat}
                    onClick={() => setTrackCategory(cat)}
                    className={`px-4 py-2 rounded-full text-sm font-medium whitespace-nowrap transition-colors ${
                      trackCategory === cat
                        ? "bg-rp-red text-white"
                        : "bg-rp-surface border border-rp-border text-rp-grey hover:text-white"
                    }`}
                  >
                    {cat}
                  </button>
                ))}
              </div>
              {/* Track list */}
              <div className="grid grid-cols-2 gap-2 max-h-[50vh] overflow-y-auto">
                {filteredTracks.map((t) => (
                  <button
                    key={t.id}
                    onClick={() => { setTrack(t); setStep("car"); }}
                    className={`p-4 rounded-lg border text-left transition-all ${
                      track?.id === t.id
                        ? "border-rp-red bg-rp-red/10"
                        : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                    }`}
                  >
                    <p className="font-semibold text-white text-sm">{t.name}</p>
                    <p className="text-xs text-rp-grey">{t.country ? `${t.country} \u2022 ` : ""}{t.category}</p>
                  </button>
                ))}
                {filteredTracks.length === 0 && (
                  <p className="col-span-2 text-center text-rp-grey py-8">No tracks found</p>
                )}
              </div>
            </div>
          )}

          {/* ─── CAR SELECT ──────────────────────────────────────── */}
          {step === "car" && (
            <div className="space-y-4">
              <input
                type="text"
                value={carSearch}
                onChange={(e) => setCarSearch(e.target.value)}
                placeholder="Search cars..."
                className="w-full px-4 py-3 bg-rp-surface border border-rp-border rounded-lg text-white placeholder:text-rp-grey focus:outline-none focus:border-rp-red"
              />
              <div className="flex gap-2 overflow-x-auto pb-2">
                {carCategories.map((cat) => (
                  <button
                    key={cat}
                    onClick={() => setCarCategory(cat)}
                    className={`px-4 py-2 rounded-full text-sm font-medium whitespace-nowrap transition-colors ${
                      carCategory === cat
                        ? "bg-rp-red text-white"
                        : "bg-rp-surface border border-rp-border text-rp-grey hover:text-white"
                    }`}
                  >
                    {cat}
                  </button>
                ))}
              </div>
              <div className="grid grid-cols-2 gap-2 max-h-[50vh] overflow-y-auto">
                {filteredCars.map((c) => (
                  <button
                    key={c.id}
                    onClick={() => { setCar(c); setStep("settings"); }}
                    className={`p-4 rounded-lg border text-left transition-all ${
                      car?.id === c.id
                        ? "border-rp-red bg-rp-red/10"
                        : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                    }`}
                  >
                    <p className="font-semibold text-white text-sm">{c.name}</p>
                    <p className="text-xs text-rp-grey">{c.category}</p>
                  </button>
                ))}
                {filteredCars.length === 0 && (
                  <p className="col-span-2 text-center text-rp-grey py-8">No cars found</p>
                )}
              </div>
            </div>
          )}

          {/* ─── DIFFICULTY & TRANSMISSION ────────────────────────── */}
          {step === "settings" && (
            <div className="space-y-8">
              <div>
                <h3 className="text-lg font-semibold text-white mb-4">Difficulty</h3>
                <div className="grid grid-cols-3 gap-4">
                  {Object.entries(DIFFICULTY_PRESETS).map(([key, preset]) => (
                    <button
                      key={key}
                      onClick={() => setDifficulty(key)}
                      className={`p-6 rounded-xl border-2 text-center transition-all ${
                        difficulty === key
                          ? "border-rp-red bg-rp-red/10"
                          : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                      }`}
                    >
                      <p className="text-xl font-bold text-white">{preset.label}</p>
                      <p className="text-xs text-rp-grey mt-2">{preset.desc}</p>
                    </button>
                  ))}
                </div>
              </div>

              <div>
                <h3 className="text-lg font-semibold text-white mb-4">Transmission</h3>
                <div className="grid grid-cols-2 gap-4">
                  <button
                    onClick={() => setTransmission("auto")}
                    className={`p-6 rounded-xl border-2 text-center transition-all ${
                      transmission === "auto"
                        ? "border-rp-red bg-rp-red/10"
                        : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                    }`}
                  >
                    <p className="text-xl font-bold text-white">Automatic</p>
                    <p className="text-xs text-rp-grey mt-1">Auto gear shifts</p>
                  </button>
                  <button
                    onClick={() => setTransmission("manual")}
                    className={`p-6 rounded-xl border-2 text-center transition-all ${
                      transmission === "manual"
                        ? "border-rp-red bg-rp-red/10"
                        : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                    }`}
                  >
                    <p className="text-xl font-bold text-white">Manual</p>
                    <p className="text-xs text-rp-grey mt-1">Paddle shifters</p>
                  </button>
                </div>
              </div>

              <div>
                <h3 className="text-lg font-semibold text-white mb-4">Force Feedback</h3>
                <div className="grid grid-cols-3 gap-4">
                  {([
                    { key: "light", label: "Light", desc: "Casual / kids" },
                    { key: "medium", label: "Medium", desc: "Balanced default" },
                    { key: "strong", label: "Strong", desc: "Full force" },
                  ] as const).map((preset) => (
                    <button
                      key={preset.key}
                      onClick={() => setFfb(preset.key)}
                      className={`p-6 rounded-xl border-2 text-center transition-all ${
                        ffb === preset.key
                          ? "border-rp-red bg-rp-red/10"
                          : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                      }`}
                    >
                      <p className="text-xl font-bold text-white">{preset.label}</p>
                      <p className="text-xs text-rp-grey mt-2">{preset.desc}</p>
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* ─── REVIEW & LAUNCH ─────────────────────────────────── */}
          {step === "review" && (
            <div className="space-y-6">
              <div className="bg-rp-surface border border-rp-border rounded-xl p-6 space-y-4">
                <div className="flex justify-between">
                  <span className="text-rp-grey">Pod</span>
                  <span className="text-white font-semibold">Rig {podNumber}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-rp-grey">Driver</span>
                  <span className="text-white font-semibold">{driverName}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-rp-grey">Game</span>
                  <span className="text-white font-semibold">{GAMES.find((g) => g.id === game)?.name}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-rp-grey">Track</span>
                  <span className="text-white font-semibold">{track?.name}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-rp-grey">Car</span>
                  <span className="text-white font-semibold">{car?.name}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-rp-grey">Difficulty</span>
                  <span className="text-white font-semibold">{DIFFICULTY_PRESETS[difficulty]?.label}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-rp-grey">Transmission</span>
                  <span className="text-white font-semibold capitalize">{transmission}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-rp-grey">Force Feedback</span>
                  <span className="text-white font-semibold capitalize">{ffb}</span>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-rp-border">
          <button
            onClick={goBack}
            className="px-6 py-3 border border-rp-border rounded-lg text-rp-grey hover:text-white hover:border-rp-red transition-colors"
          >
            Back
          </button>

          {step === "settings" && (
            <button
              onClick={() => setStep("review")}
              className="px-8 py-3 bg-rp-red hover:bg-rp-red-hover text-white font-bold rounded-lg transition-colors"
            >
              Review
            </button>
          )}

          {step === "review" && (
            <button
              onClick={handleLaunch}
              className="px-10 py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-lg transition-colors"
            >
              LAUNCH
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
