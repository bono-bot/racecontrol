"use client";

import { useState, useEffect, useMemo } from "react";
import { api } from "@/lib/api";
import { GAMES, GAME_LABELS, CLASS_COLORS, DIFFICULTY_PRESETS } from "@/lib/constants";
import type { Driver, PricingTier, AcCatalog, CatalogItem, KioskExperience, SessionType } from "@/lib/types";
import type { WizardState } from "@/hooks/useSetupWizard";

interface SetupWizardProps {
  podId: string;
  podNumber: number;
  wizardState: WizardState;
  setField: <K extends keyof WizardState>(key: K, value: WizardState[K]) => void;
  goToStep: (step: WizardState["currentStep"]) => void;
  goBack: () => boolean;
  goNext: () => boolean;
  isFirstStep: boolean;
  onLaunch: (simType: string, launchArgs: string) => void;
  buildLaunchArgs: () => string;
  onCancel: () => void;
}

const STEP_TITLES: Record<string, string> = {
  register_driver: "Register Driver",
  select_plan: "Select Plan",
  select_game: "Select Game",
  session_splits: "Session Format",
  player_mode: "Player Mode",
  session_type: "Session Type",
  ai_config: "AI Opponents",
  multiplayer_lobby: "Multiplayer",
  select_experience: "Select Experience",
  select_track: "Select Track",
  select_car: "Select Car",
  driving_settings: "Driving Settings",
  review: "Review & Launch",
};

export function SetupWizard({
  podId,
  podNumber,
  wizardState: ws,
  setField,
  goToStep,
  goBack,
  goNext,
  isFirstStep,
  onLaunch,
  buildLaunchArgs,
  onCancel,
}: SetupWizardProps) {
  // ─── Shared Data ─────────────────────────────────────────────────────
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [tiers, setTiers] = useState<PricingTier[]>([]);
  const [catalog, setCatalog] = useState<AcCatalog | null>(null);
  const [experiences, setExperiences] = useState<KioskExperience[]>([]);
  const [walletCache, setWalletCache] = useState<Map<string, number>>(new Map());

  // Split options
  const [splitOptions, setSplitOptions] = useState<{ count: number; duration_minutes: number; label: string }[]>([]);

  // Search/filter state
  const [searchQuery, setSearchQuery] = useState("");
  const [driverName, setDriverName] = useState("");
  const [driverPhone, setDriverPhone] = useState("");
  const [trackSearch, setTrackSearch] = useState("");
  const [trackCategory, setTrackCategory] = useState("Featured");
  const [carSearch, setCarSearch] = useState("");
  const [carCategory, setCarCategory] = useState("Featured");

  // Fetch split options when entering session_splits step
  useEffect(() => {
    if (ws.currentStep === "session_splits" && ws.selectedTier) {
      api.getSplitOptions(ws.selectedTier.duration_minutes).then((res) => {
        setSplitOptions(res.options || []);
      }).catch(() => setSplitOptions([]));
    }
  }, [ws.currentStep, ws.selectedTier]);

  // Load data on mount
  useEffect(() => {
    api.listDrivers().then((res) => setDrivers(res.drivers || []));
    api.listPricingTiers().then((res) => setTiers((res.tiers || []).filter((t) => t.is_active)));
    api.getAcCatalog().then((data) => setCatalog(data)).catch(() => {});
    api.listExperiences().then((res) => setExperiences((res.experiences || []).filter((e) => e.is_active)));
  }, []);

  // Filtered drivers
  const filteredDrivers = searchQuery.length >= 2
    ? drivers.filter(
        (d) =>
          d.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
          (d.phone && d.phone.includes(searchQuery))
      )
    : [];

  // Fetch wallet balances for search results
  useEffect(() => {
    for (const d of filteredDrivers) {
      if (!walletCache.has(d.id)) {
        api.getWallet(d.id).then((res) => {
          if (res.wallet) {
            setWalletCache((prev) => new Map(prev).set(d.id, res.wallet!.balance_paise));
          }
        }).catch(() => {});
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filteredDrivers.map((d) => d.id).join(",")]);

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
    // Filter by session type: AI-requiring types only show tracks with AI
    const aiTypes = ["race", "trackday", "race_weekend"];
    if (aiTypes.includes(ws.sessionType)) {
      items = items.filter((t) => {
        const available = (t as unknown as Record<string, unknown>).available_session_types as string[] | undefined;
        return !available || available.includes(ws.sessionType);
      });
    }
    return items;
  }, [catalog, trackCategory, trackSearch, ws.sessionType]);

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

  // ─── Handlers ────────────────────────────────────────────────────────
  async function handleCreateDriver() {
    if (!driverName.trim()) return;
    const result = await api.createDriver({
      name: driverName.trim(),
      phone: driverPhone.trim() || undefined,
    });
    if (result.id) {
      setField("selectedDriver", { id: result.id, name: result.name, total_laps: 0, total_time_ms: 0 });
      goNext();
    }
  }

  function handleSelectDriver(driver: Driver) {
    setField("selectedDriver", driver);
    goNext();
  }

  function handleSelectTier(tier: PricingTier) {
    setField("selectedTier", tier);
    goNext();
  }

  function handleSelectGame(gameId: string) {
    setField("selectedGame", gameId);
    goNext();
  }

  function handleSelectSplit(count: number, durationMinutes: number) {
    setField("splitCount", count);
    setField("splitDurationMinutes", count > 1 ? durationMinutes : null);
    goNext();
  }

  function handleSelectPlayerMode(mode: "single" | "multi") {
    setField("playerMode", mode);
    goNext();
  }

  function handleSelectSessionType(type: SessionType) {
    setField("sessionType", type);
    goNext();
  }

  function handleSelectExperience(exp: KioskExperience) {
    setField("selectedExperience", exp);
    // Non-AC games skip driving_settings — go straight to review
    const isAc = ws.selectedGame === "assetto_corsa";
    goToStep(isAc ? "driving_settings" : "review");
  }

  function handleSelectTrack(track: CatalogItem) {
    setField("selectedTrack", track);
    goNext();
  }

  function handleSelectCar(car: CatalogItem) {
    setField("selectedCar", car);
    goNext();
  }

  function handleLaunch() {
    const launchArgs = buildLaunchArgs();
    onLaunch(ws.selectedGame, launchArgs);
  }

  const step = ws.currentStep;

  return (
    <div className="flex flex-col h-full">
      {/* Step Header */}
      <div className="px-5 py-3 border-b border-rp-border bg-rp-card/50">
        <div className="flex items-center gap-2 text-xs text-rp-grey">
          <span>Pod {podNumber}</span>
          {ws.selectedDriver && (
            <>
              <span>&middot;</span>
              <span className="text-white">{ws.selectedDriver.name}</span>
            </>
          )}
          {ws.selectedTier && (
            <>
              <span>&middot;</span>
              <span>{ws.selectedTier.name}</span>
            </>
          )}
        </div>
        <h3 data-testid="wizard-step-title" className="text-lg font-bold text-white mt-1">{STEP_TITLES[step] || step}</h3>
      </div>

      {/* Step Content */}
      <div className="flex-1 overflow-y-auto p-5">
        {/* ─── REGISTER DRIVER ──────────────────────────────────── */}
        {step === "register_driver" && (
          <div data-testid="step-register-driver" className="space-y-4">
            <div>
              <label className="text-xs text-rp-grey uppercase tracking-wider block mb-1">
                Search Existing Driver
              </label>
              <input
                data-testid="driver-search"
                type="text"
                placeholder="Name or phone..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full px-3 py-2 bg-rp-surface border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
              />
              {filteredDrivers.length > 0 && (
                <div className="mt-1 max-h-48 overflow-y-auto border border-rp-border rounded bg-rp-surface">
                  {filteredDrivers.slice(0, 8).map((d) => {
                    const bal = walletCache.get(d.id);
                    const last4 = d.phone ? d.phone.slice(-4) : null;
                    return (
                      <button
                        key={d.id}
                        data-testid={`driver-result-${d.id}`}
                        onClick={() => handleSelectDriver(d)}
                        className="w-full text-left px-3 py-2 hover:bg-rp-red/10 text-sm flex items-center justify-between"
                      >
                        <div>
                          <span className="text-white">{d.name}</span>
                          {last4 && <span className="text-rp-grey text-xs ml-2">****{last4}</span>}
                        </div>
                        <span className={`text-xs font-medium ${bal !== undefined && bal > 0 ? "text-emerald-400" : "text-rp-grey"}`}>
                          {bal !== undefined ? `${(bal / 100).toFixed(0)} cr` : "\u2014"}
                        </span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>

            <div className="flex items-center gap-3 text-xs text-rp-grey">
              <div className="flex-1 h-px bg-rp-border" />
              <span>or create new</span>
              <div className="flex-1 h-px bg-rp-border" />
            </div>

            <div className="space-y-3">
              <input
                data-testid="new-driver-name"
                type="text"
                placeholder="Driver name *"
                value={driverName}
                onChange={(e) => setDriverName(e.target.value)}
                className="w-full px-3 py-2 bg-rp-surface border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
              />
              <input
                type="text"
                placeholder="Phone (optional)"
                value={driverPhone}
                onChange={(e) => setDriverPhone(e.target.value)}
                className="w-full px-3 py-2 bg-rp-surface border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
              />
              <button
                data-testid="create-driver-btn"
                onClick={handleCreateDriver}
                disabled={!driverName.trim()}
                className="w-full py-2.5 bg-rp-red hover:bg-rp-red-hover disabled:opacity-40 disabled:cursor-not-allowed text-white font-semibold rounded text-sm transition-colors"
              >
                Continue
              </button>
            </div>
          </div>
        )}

        {/* ─── SELECT PLAN ──────────────────────────────────────── */}
        {step === "select_plan" && (
          <div data-testid="step-select-plan" className="space-y-2">
            {tiers.filter(t => !t.is_trial || !ws.selectedDriver?.has_used_trial).map((tier) => (
              <button
                key={tier.id}
                data-testid={`tier-option-${tier.id}`}
                onClick={() => handleSelectTier(tier)}
                className="w-full flex items-center justify-between px-4 py-3 bg-rp-surface border border-rp-border rounded hover:border-rp-red/50 transition-colors"
              >
                <div className="text-left">
                  <p className="text-sm font-semibold text-white">{tier.name}</p>
                  <p className="text-xs text-rp-grey">{tier.duration_minutes} min</p>
                </div>
                <span className="text-sm font-bold text-rp-red">
                  {tier.is_trial ? "Free" : `${(tier.price_paise / 100).toFixed(0)} credits`}
                </span>
              </button>
            ))}
          </div>
        )}

        {/* ─── SELECT GAME ──────────────────────────────────────── */}
        {step === "select_game" && (
          <div data-testid="step-select-game" className="grid grid-cols-2 gap-3">
            {GAMES.map((g) => (
              <button
                key={g.id}
                data-testid={`game-option-${g.id}`}
                disabled={!g.enabled}
                onClick={() => handleSelectGame(g.id)}
                className={`p-5 rounded-xl border-2 text-center transition-all ${
                  g.enabled
                    ? "border-rp-border bg-rp-surface hover:border-rp-red hover:bg-rp-red/10 cursor-pointer"
                    : "border-rp-border/50 bg-rp-surface/50 opacity-40 cursor-not-allowed"
                }`}
              >
                <p className="text-base font-bold text-white">{g.name}</p>
                {!g.enabled && <p className="text-xs text-rp-grey mt-1">Coming Soon</p>}
              </button>
            ))}
          </div>
        )}

        {/* ─── SESSION SPLITS (AC only) ────────────────────────── */}
        {step === "session_splits" && (
          <div data-testid="step-session-splits" className="space-y-4">
            <p className="text-sm text-rp-grey">
              Split your {ws.selectedTier?.duration_minutes}-minute session into shorter sub-sessions.
              The game restarts between each split.
            </p>
            <div className="space-y-2">
              {splitOptions.map((opt) => (
                <button
                  key={opt.count}
                  data-testid={`split-option-${opt.count}`}
                  onClick={() => handleSelectSplit(opt.count, opt.duration_minutes)}
                  className={`w-full flex items-center justify-between px-4 py-3 bg-rp-surface border rounded transition-colors ${
                    ws.splitCount === opt.count
                      ? "border-rp-red bg-rp-red/10"
                      : "border-rp-border hover:border-rp-red/50"
                  }`}
                >
                  <div className="text-left">
                    <p className="text-sm font-semibold text-white">{opt.label}</p>
                    {opt.count === 1 && (
                      <p className="text-xs text-rp-grey">Full session, no restarts</p>
                    )}
                    {opt.count > 1 && (
                      <p className="text-xs text-rp-grey">
                        {opt.count} races of {opt.duration_minutes} min each
                      </p>
                    )}
                  </div>
                  {opt.count > 1 && (
                    <span className="text-xs text-rp-red font-medium">
                      {opt.count} races
                    </span>
                  )}
                </button>
              ))}
              {splitOptions.length === 0 && (
                <div className="text-center py-8">
                  <p className="text-sm text-rp-grey">Loading split options...</p>
                </div>
              )}
            </div>
          </div>
        )}

        {/* ─── PLAYER MODE ──────────────────────────────────────── */}
        {step === "player_mode" && (
          <div data-testid="step-player-mode" className="grid grid-cols-2 gap-4">
            <button
              data-testid="player-mode-single"
              onClick={() => handleSelectPlayerMode("single")}
              className="p-6 rounded-xl border-2 border-rp-border bg-rp-surface hover:border-rp-red hover:bg-rp-red/10 transition-all text-center"
            >
              <svg className="w-10 h-10 mx-auto mb-2 text-rp-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
              </svg>
              <p className="text-lg font-bold text-white">Singleplayer</p>
              <p className="text-xs text-rp-grey mt-1">Practice &amp; hot laps</p>
            </button>
            <button
              data-testid="player-mode-multi"
              onClick={() => handleSelectPlayerMode("multi")}
              className="p-6 rounded-xl border-2 border-rp-border bg-rp-surface hover:border-rp-red hover:bg-rp-red/10 transition-all text-center"
            >
              <svg className="w-10 h-10 mx-auto mb-2 text-rp-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z" />
              </svg>
              <p className="text-lg font-bold text-white">Multiplayer</p>
              <p className="text-xs text-rp-grey mt-1">Race with others</p>
            </button>
          </div>
        )}

        {/* ─── SESSION TYPE ─────────────────────────────────────── */}
        {step === "session_type" && (
          <div data-testid="step-session-type" className="space-y-3">
            {([
              { type: "practice" as const, label: "Practice", desc: "Free driving, no AI, no timer", icon: "M13 10V3L4 14h7v7l9-11h-7z" },
              { type: "hotlap" as const, label: "Hotlap", desc: "Timed laps -- set the fastest time", icon: "M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" },
              { type: "race" as const, label: "Race vs AI", desc: "Full grid race against AI opponents", icon: "M3 21v-4m0 0V5a2 2 0 012-2h6.5l1 1H21l-3 6 3 6h-8.5l-1-1H5a2 2 0 00-2 2zm9-13.5V9" },
              { type: "trackday" as const, label: "Track Day", desc: "Open pit, mixed traffic on track", icon: "M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l5.447 2.724A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7" },
              { type: "race_weekend" as const, label: "Race Weekend", desc: "Practice, Qualify, then Race sequence", icon: "M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" },
            ]).map(({ type, label, desc, icon }) => (
              <button
                key={type}
                data-testid={`session-type-${type}`}
                onClick={() => handleSelectSessionType(type)}
                className={`w-full flex items-center gap-4 px-5 py-4 rounded-xl border-2 transition-all text-left ${
                  ws.sessionType === type
                    ? "border-rp-red bg-rp-red/10"
                    : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                }`}
              >
                <svg className="w-8 h-8 text-rp-red shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d={icon} />
                </svg>
                <div>
                  <p className="text-base font-bold text-white">{label}</p>
                  <p className="text-xs text-rp-grey">{desc}</p>
                </div>
              </button>
            ))}
          </div>
        )}

        {/* ─── AI CONFIG ────────────────────────────────────────── */}
        {step === "ai_config" && (
          <div data-testid="step-ai-config" className="space-y-6">
            {/* AI Toggle */}
            <div className="flex items-center justify-between px-4 py-3 bg-rp-surface border border-rp-border rounded-xl">
              <div>
                <p className="text-sm font-semibold text-white">AI Opponents</p>
                <p className="text-xs text-rp-grey">Race against computer-controlled drivers</p>
              </div>
              <button
                data-testid="ai-toggle"
                onClick={() => setField("aiEnabled", !ws.aiEnabled)}
                className={`w-12 h-6 rounded-full transition-colors ${ws.aiEnabled ? "bg-rp-red" : "bg-zinc-700"}`}
              >
                <div className={`w-5 h-5 bg-white rounded-full transition-transform mx-0.5 ${ws.aiEnabled ? "translate-x-6" : ""}`} />
              </button>
            </div>

            {ws.aiEnabled && (
              <>
                {/* AI Difficulty */}
                <div>
                  <h4 className="text-sm font-semibold text-white mb-3">AI Difficulty</h4>
                  <div className="grid grid-cols-3 gap-3">
                    {(["easy", "medium", "hard"] as const).map((level) => (
                      <button
                        key={level}
                        data-testid={`ai-difficulty-${level}`}
                        onClick={() => setField("aiDifficulty", level)}
                        className={`p-4 rounded-xl border-2 text-center transition-all ${
                          ws.aiDifficulty === level
                            ? "border-rp-red bg-rp-red/10"
                            : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                        }`}
                      >
                        <p className="text-sm font-bold text-white capitalize">{level}</p>
                      </button>
                    ))}
                  </div>
                </div>

                {/* AI Count */}
                <div>
                  <h4 className="text-sm font-semibold text-white mb-3">
                    Number of AI: <span className="text-rp-red">{ws.aiCount}</span>
                  </h4>
                  <input
                    data-testid="ai-count-slider"
                    type="range"
                    min={1}
                    max={20}
                    value={ws.aiCount}
                    onChange={(e) => setField("aiCount", parseInt(e.target.value))}
                    className="w-full accent-rp-red"
                  />
                  <div className="flex justify-between text-xs text-rp-grey mt-1">
                    <span>1</span>
                    <span>20</span>
                  </div>
                </div>
              </>
            )}

            {/* Next button */}
            <button
              data-testid="ai-config-next"
              onClick={goNext}
              className="w-full py-3 bg-rp-red hover:bg-rp-red-hover text-white font-bold rounded-lg transition-colors"
            >
              Continue
            </button>
          </div>
        )}

        {/* ─── MULTIPLAYER LOBBY ────────────────────────────────── */}
        {step === "multiplayer_lobby" && (
          <div data-testid="step-multiplayer-lobby" className="space-y-6">
            {/* Create or Join toggle */}
            <div className="grid grid-cols-2 gap-3">
              <button
                onClick={() => setField("multiplayerMode", "create")}
                className={`p-4 rounded-xl border-2 text-center transition-all ${
                  ws.multiplayerMode === "create"
                    ? "border-rp-red bg-rp-red/10"
                    : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                }`}
              >
                <svg className="w-8 h-8 mx-auto mb-2 text-rp-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6v6m0 0v6m0-6h6m-6 0H6" />
                </svg>
                <p className="text-sm font-bold text-white">Create Server</p>
                <p className="text-xs text-rp-grey mt-1">Host a race lobby</p>
              </button>
              <button
                onClick={() => setField("multiplayerMode", "join")}
                className={`p-4 rounded-xl border-2 text-center transition-all ${
                  ws.multiplayerMode === "join"
                    ? "border-rp-red bg-rp-red/10"
                    : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                }`}
              >
                <svg className="w-8 h-8 mx-auto mb-2 text-rp-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 16l-4-4m0 0l4-4m-4 4h14m-5 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h7a3 3 0 013 3v1" />
                </svg>
                <p className="text-sm font-bold text-white">Join Server</p>
                <p className="text-xs text-rp-grey mt-1">Join an existing lobby</p>
              </button>
            </div>

            {/* Join server form */}
            {ws.multiplayerMode === "join" && (
              <div className="space-y-3">
                <div>
                  <label className="text-xs text-rp-grey uppercase tracking-wider block mb-1">Server IP</label>
                  <input
                    type="text"
                    placeholder="192.168.31.23"
                    value={ws.serverIp}
                    onChange={(e) => setField("serverIp", e.target.value)}
                    className="w-full px-3 py-2 bg-rp-surface border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
                  />
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="text-xs text-rp-grey uppercase tracking-wider block mb-1">Port</label>
                    <input
                      type="text"
                      placeholder="9600"
                      value={ws.serverPort}
                      onChange={(e) => setField("serverPort", e.target.value)}
                      className="w-full px-3 py-2 bg-rp-surface border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
                    />
                  </div>
                  <div>
                    <label className="text-xs text-rp-grey uppercase tracking-wider block mb-1">HTTP Port</label>
                    <input
                      type="text"
                      placeholder="8081"
                      value={ws.serverHttpPort}
                      onChange={(e) => setField("serverHttpPort", e.target.value)}
                      className="w-full px-3 py-2 bg-rp-surface border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
                    />
                  </div>
                </div>
                <div>
                  <label className="text-xs text-rp-grey uppercase tracking-wider block mb-1">Password (optional)</label>
                  <input
                    type="text"
                    placeholder="Server password"
                    value={ws.serverPassword}
                    onChange={(e) => setField("serverPassword", e.target.value)}
                    className="w-full px-3 py-2 bg-rp-surface border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
                  />
                </div>
              </div>
            )}

            {/* Create server info */}
            {ws.multiplayerMode === "create" && (
              <div className="bg-rp-surface border border-rp-border rounded-xl p-4">
                <p className="text-sm text-rp-grey">
                  A dedicated server will be started on Racing-Point-Server.
                  Other pods can join using the server details shown after creation.
                </p>
              </div>
            )}

            {ws.multiplayerMode && (
              <button
                onClick={goNext}
                disabled={ws.multiplayerMode === "join" && !ws.serverIp}
                className="w-full py-3 bg-rp-red hover:bg-rp-red-hover disabled:opacity-40 text-white font-bold rounded-lg transition-colors"
              >
                Continue
              </button>
            )}
          </div>
        )}

        {/* ─── SELECT EXPERIENCE (preset) ───────────────────────── */}
        {step === "select_experience" && (() => {
          const isAc = ws.selectedGame === "assetto_corsa";
          // Filter experiences by selected game
          const gameExperiences = experiences.filter((e) => e.game === ws.selectedGame);
          return (
          <div data-testid="step-select-experience" className="space-y-4">
            {/* Mode toggle — only show Custom option for AC (has track/car catalog) */}
            {isAc && (
            <div className="flex gap-2">
              <button
                data-testid="experience-mode-preset"
                onClick={() => setField("experienceMode", "preset")}
                className={`flex-1 py-2 rounded-lg text-sm font-medium transition-colors ${
                  ws.experienceMode === "preset"
                    ? "bg-rp-red text-white"
                    : "bg-rp-surface border border-rp-border text-rp-grey hover:text-white"
                }`}
              >
                Preset Experiences
              </button>
              <button
                data-testid="experience-mode-custom"
                onClick={() => {
                  setField("experienceMode", "custom");
                  goToStep("select_track");
                }}
                className="flex-1 py-2 rounded-lg text-sm font-medium transition-colors bg-rp-surface border border-rp-border text-rp-grey hover:text-white"
              >
                Custom (Track + Car)
              </button>
            </div>
            )}

            {/* Non-AC: show hint that game handles config internally */}
            {!isAc && (
              <p className="text-xs text-rp-grey text-center">
                Choose a duration below. Track, car, and settings are configured in-game.
              </p>
            )}

            {/* Experience list — filtered by selected game */}
            <div data-testid="experience-list" className="space-y-2 max-h-[50vh] overflow-y-auto">
              {gameExperiences.length === 0 ? (
                <p className="text-sm text-rp-grey text-center py-8">No experiences configured for {GAME_LABELS[ws.selectedGame] || ws.selectedGame}</p>
              ) : (
                gameExperiences.map((exp) => (
                  <button
                    key={exp.id}
                    data-testid={`experience-option-${exp.id}`}
                    onClick={() => handleSelectExperience(exp)}
                    className="w-full flex items-center gap-3 px-4 py-3 bg-rp-surface border border-rp-border rounded hover:border-rp-red/50 transition-colors text-left"
                  >
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
                      {isAc && (
                      <p className="text-xs text-rp-grey truncate">
                        {exp.track} &middot; {exp.car}
                      </p>
                      )}
                    </div>
                    <div className="text-right shrink-0">
                      <p className="text-xs text-rp-grey">
                        {exp.duration_minutes}min
                      </p>
                    </div>
                  </button>
                ))
              )}
            </div>
          </div>
          );
        })()}

        {/* ─── SELECT TRACK ─────────────────────────────────────── */}
        {step === "select_track" && (
          <div data-testid="step-select-track" className="space-y-4">
            <input
              data-testid="track-search"
              type="text"
              value={trackSearch}
              onChange={(e) => setTrackSearch(e.target.value)}
              placeholder="Search tracks..."
              className="w-full px-4 py-3 bg-rp-surface border border-rp-border rounded-lg text-white placeholder:text-rp-grey focus:outline-none focus:border-rp-red"
            />
            <div className="flex gap-2 overflow-x-auto pb-2">
              {trackCategories.map((cat) => (
                <button
                  key={cat}
                  onClick={() => setTrackCategory(cat)}
                  className={`px-3 py-1.5 rounded-full text-xs font-medium whitespace-nowrap transition-colors ${
                    trackCategory === cat
                      ? "bg-rp-red text-white"
                      : "bg-rp-surface border border-rp-border text-rp-grey hover:text-white"
                  }`}
                >
                  {cat}
                </button>
              ))}
            </div>
            <div className="grid grid-cols-2 gap-2 max-h-[50vh] overflow-y-auto">
              {filteredTracks.map((t) => (
                <button
                  key={t.id}
                  data-testid={`track-option-${t.id}`}
                  onClick={() => handleSelectTrack(t)}
                  className={`p-3 rounded-lg border text-left transition-all ${
                    ws.selectedTrack?.id === t.id
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

        {/* ─── SELECT CAR ───────────────────────────────────────── */}
        {step === "select_car" && (
          <div data-testid="step-select-car" className="space-y-4">
            <input
              data-testid="car-search"
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
                  className={`px-3 py-1.5 rounded-full text-xs font-medium whitespace-nowrap transition-colors ${
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
                  data-testid={`car-option-${c.id}`}
                  onClick={() => handleSelectCar(c)}
                  className={`p-3 rounded-lg border text-left transition-all ${
                    ws.selectedCar?.id === c.id
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

        {/* ─── DRIVING SETTINGS ─────────────────────────────────── */}
        {step === "driving_settings" && (
          <div data-testid="step-driving-settings" className="space-y-6">
            {/* Difficulty */}
            <div>
              <h4 className="text-sm font-semibold text-white mb-3">Difficulty</h4>
              <div className="grid grid-cols-3 gap-3">
                {Object.entries(DIFFICULTY_PRESETS).map(([key, preset]) => (
                  <button
                    key={key}
                    data-testid={`difficulty-${key}`}
                    onClick={() => setField("drivingDifficulty", key)}
                    className={`p-4 rounded-xl border-2 text-center transition-all ${
                      ws.drivingDifficulty === key
                        ? "border-rp-red bg-rp-red/10"
                        : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                    }`}
                  >
                    <p className="text-sm font-bold text-white">{preset.label}</p>
                    <p className="text-[10px] text-rp-grey mt-1">{preset.desc}</p>
                  </button>
                ))}
              </div>
            </div>

            {/* Transmission */}
            <div>
              <h4 className="text-sm font-semibold text-white mb-3">Transmission</h4>
              <div className="grid grid-cols-2 gap-3">
                {([
                  { key: "auto", label: "Automatic", desc: "Auto gear shifts" },
                  { key: "manual", label: "Manual", desc: "Paddle shifters" },
                ] as const).map(({ key, label, desc }) => (
                  <button
                    key={key}
                    data-testid={`transmission-${key}`}
                    onClick={() => setField("transmission", key)}
                    className={`p-4 rounded-xl border-2 text-center transition-all ${
                      ws.transmission === key
                        ? "border-rp-red bg-rp-red/10"
                        : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                    }`}
                  >
                    <p className="text-sm font-bold text-white">{label}</p>
                    <p className="text-[10px] text-rp-grey mt-1">{desc}</p>
                  </button>
                ))}
              </div>
            </div>

            {/* FFB */}
            <div>
              <h4 className="text-sm font-semibold text-white mb-3">Force Feedback</h4>
              <div className="grid grid-cols-3 gap-3">
                {([
                  { key: "light", label: "Light", desc: "Casual / kids" },
                  { key: "medium", label: "Medium", desc: "Balanced default" },
                  { key: "strong", label: "Strong", desc: "Full force" },
                ] as const).map(({ key, label, desc }) => (
                  <button
                    key={key}
                    data-testid={`ffb-${key}`}
                    onClick={() => setField("ffb", key)}
                    className={`p-4 rounded-xl border-2 text-center transition-all ${
                      ws.ffb === key
                        ? "border-rp-red bg-rp-red/10"
                        : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                    }`}
                  >
                    <p className="text-sm font-bold text-white">{label}</p>
                    <p className="text-[10px] text-rp-grey mt-1">{desc}</p>
                  </button>
                ))}
              </div>
            </div>

            <button
              data-testid="driving-settings-next"
              onClick={goNext}
              className="w-full py-3 bg-rp-red hover:bg-rp-red-hover text-white font-bold rounded-lg transition-colors"
            >
              Review
            </button>
          </div>
        )}

        {/* ─── REVIEW & LAUNCH ──────────────────────────────────── */}
        {step === "review" && (
          <div data-testid="step-review" className="space-y-4">
            <div className="bg-rp-surface border border-rp-border rounded-xl p-5 space-y-3">
              <ReviewRow label="Pod" value={`Rig ${podNumber}`} />
              <ReviewRow label="Driver" value={ws.selectedDriver?.name || ""} />
              <ReviewRow label="Plan" value={ws.selectedTier?.name || ""} />
              <ReviewRow label="Game" value={GAME_LABELS[ws.selectedGame] || ws.selectedGame} />
              {ws.splitCount > 1 && ws.splitDurationMinutes && (
                <ReviewRow label="Format" value={`${ws.splitCount} × ${ws.splitDurationMinutes} min`} />
              )}
              <ReviewRow label="Mode" value={ws.playerMode === "multi" ? "Multiplayer" : "Singleplayer"} />
              <ReviewRow label="Session" value={ws.sessionType.charAt(0).toUpperCase() + ws.sessionType.slice(1)} />
              {ws.aiEnabled && (
                <>
                  <ReviewRow label="AI Opponents" value={`${ws.aiCount} (${ws.aiDifficulty})`} />
                </>
              )}
              {ws.selectedExperience ? (
                <ReviewRow label="Experience" value={ws.selectedExperience.name} />
              ) : (
                <>
                  <ReviewRow label="Track" value={ws.selectedTrack?.name || ""} />
                  <ReviewRow label="Car" value={ws.selectedCar?.name || ""} />
                </>
              )}
              <ReviewRow label="Difficulty" value={DIFFICULTY_PRESETS[ws.drivingDifficulty]?.label || ws.drivingDifficulty} />
              <ReviewRow label="Transmission" value={ws.transmission === "auto" ? "Automatic" : "Manual"} />
              <ReviewRow label="FFB" value={ws.ffb.charAt(0).toUpperCase() + ws.ffb.slice(1)} />
              {ws.playerMode === "multi" && ws.multiplayerMode === "join" && (
                <ReviewRow label="Server" value={`${ws.serverIp}:${ws.serverPort}`} />
              )}
              {ws.playerMode === "multi" && ws.multiplayerMode === "create" && (
                <ReviewRow label="Server" value="Create new (Racing-Point-Server)" />
              )}
            </div>

            <button
              data-testid="launch-btn"
              onClick={handleLaunch}
              className="w-full py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-xl transition-colors"
            >
              LAUNCH
            </button>
          </div>
        )}
      </div>

      {/* Footer with Back button */}
      {!isFirstStep && step !== "review" && (
        <div className="px-5 py-3 border-t border-rp-border shrink-0">
          <button
            data-testid="wizard-back-btn"
            onClick={() => { if (!goBack()) onCancel(); }}
            className="px-4 py-2 border border-rp-border rounded-lg text-sm text-rp-grey hover:text-white hover:border-rp-red transition-colors"
          >
            Back
          </button>
        </div>
      )}
      {step === "review" && (
        <div className="px-5 py-3 border-t border-rp-border shrink-0">
          <button
            data-testid="wizard-back-btn"
            onClick={() => goBack()}
            className="px-4 py-2 border border-rp-border rounded-lg text-sm text-rp-grey hover:text-white hover:border-rp-red transition-colors"
          >
            Back
          </button>
        </div>
      )}
    </div>
  );
}

function ReviewRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between">
      <span className="text-rp-grey text-sm">{label}</span>
      <span className="text-white font-semibold text-sm">{value}</span>
    </div>
  );
}
