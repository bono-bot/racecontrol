"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import { useSetupWizard } from "@/hooks/useSetupWizard";
import type { PricingTier, AcCatalog, CatalogItem, KioskExperience } from "@/lib/types";

// ─── Phase Definitions ──────────────────────────────────────────────────────

type Phase = "phone" | "otp" | "wizard" | "booking" | "success" | "error";

// ─── Constants ──────────────────────────────────────────────────────────────

const AUTO_RETURN_MS = 30_000;
const INACTIVITY_MS = 120_000;

const DIFFICULTY_PRESETS: Record<string, { label: string; desc: string }> = {
  easy: { label: "Easy", desc: "ABS, TC, Stability, Ideal Line" },
  medium: { label: "Medium", desc: "ABS, TC only" },
  hard: { label: "Hard", desc: "No assists" },
};

const GAMES = [
  { id: "assetto_corsa", name: "Assetto Corsa", enabled: true },
  { id: "assetto_corsa_evo", name: "AC EVO", enabled: true },
  { id: "assetto_corsa_rally", name: "AC Rally", enabled: true },
  { id: "f1_25", name: "F1 25", enabled: true },
  { id: "iracing", name: "iRacing", enabled: true },
  { id: "le_mans_ultimate", name: "Le Mans Ultimate", enabled: true },
  { id: "forza", name: "Forza Motorsport", enabled: false },
  { id: "forza_horizon_5", name: "Forza Horizon 5", enabled: true },
];

const GAME_LABELS: Record<string, string> = {
  assetto_corsa: "Assetto Corsa",
  assetto_corsa_evo: "AC EVO",
  assetto_corsa_rally: "AC Rally",
  f1_25: "F1 25",
  iracing: "iRacing",
  le_mans_ultimate: "Le Mans Ultimate",
  forza: "Forza Motorsport",
  forza_horizon_5: "Forza Horizon 5",
};

const CLASS_COLORS: Record<string, string> = {
  A: "bg-rp-red text-white",
  B: "bg-orange-500 text-white",
  C: "bg-amber-500 text-black",
  D: "bg-green-500 text-white",
};

const STEP_TITLES: Record<string, string> = {
  select_plan: "Select Plan",
  select_game: "Select Game",
  player_mode: "Player Mode",
  session_type: "Session Type",
  ai_config: "AI Opponents",
  multiplayer_lobby: "Multiplayer",
  select_experience: "Select Experience",
  select_track: "Select Track",
  select_car: "Select Car",
  driving_settings: "Driving Settings",
  review: "Review & Book",
};

// ─── Main Booking Page ──────────────────────────────────────────────────────

export default function BookingPage() {
  const router = useRouter();
  const wizard = useSetupWizard();

  // ─── Auth state ────────────────────────────────────────────────────────
  const [phase, setPhase] = useState<Phase>("phone");
  const [phone, setPhone] = useState("");
  const [otp, setOtp] = useState("");
  const [authToken, setAuthToken] = useState("");
  const [driverName, setDriverName] = useState("");
  const [driverId, setDriverId] = useState("");
  const [errorMsg, setErrorMsg] = useState("");
  const [loading, setLoading] = useState(false);
  const [lastActivity, setLastActivity] = useState(Date.now());

  // ─── Success state ─────────────────────────────────────────────────────
  const [resultPin, setResultPin] = useState("");
  const [resultPodNumber, setResultPodNumber] = useState(0);
  const [resultAllocatedSeconds, setResultAllocatedSeconds] = useState(0);

  // ─── Wizard data ───────────────────────────────────────────────────────
  const [tiers, setTiers] = useState<PricingTier[]>([]);
  const [catalog, setCatalog] = useState<AcCatalog | null>(null);
  const [experiences, setExperiences] = useState<KioskExperience[]>([]);
  const [trackSearch, setTrackSearch] = useState("");
  const [trackCategory, setTrackCategory] = useState("Featured");
  const [carSearch, setCarSearch] = useState("");
  const [carCategory, setCarCategory] = useState("Featured");

  const touch = useCallback(() => setLastActivity(Date.now()), []);

  // Load wizard data on mount
  useEffect(() => {
    api.listPricingTiers().then((res) => setTiers((res.tiers || []).filter((t) => t.is_active)));
    api.getAcCatalog().then((data) => setCatalog(data)).catch(() => {});
    api.listExperiences().then((res) => setExperiences((res.experiences || []).filter((e) => e.is_active)));
  }, []);

  // Start wizard at select_plan (skip register_driver — handled by phone auth)
  useEffect(() => {
    if (phase === "wizard" && wizard.state.currentStep === "register_driver") {
      wizard.goToStep("select_plan");
    }
  }, [phase, wizard.state.currentStep, wizard]);

  // ─── Auto-return timers ────────────────────────────────────────────────

  // Success screen auto-returns to walk-in
  useEffect(() => {
    if (phase !== "success") return;
    const timer = setTimeout(() => router.push("/"), AUTO_RETURN_MS);
    return () => clearTimeout(timer);
  }, [phase, router]);

  // Inactivity auto-returns during phone/otp entry
  useEffect(() => {
    if (phase !== "phone" && phase !== "otp") return;
    const interval = setInterval(() => {
      if (Date.now() - lastActivity > INACTIVITY_MS) {
        router.push("/");
      }
    }, 5000);
    return () => clearInterval(interval);
  }, [phase, lastActivity, router]);

  // ─── Filtered tracks/cars ──────────────────────────────────────────────

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

  // ─── Phone digit handlers (reuse numpad style) ─────────────────────────

  function handlePhoneDigit(digit: string) {
    touch();
    if (phone.length < 10) setPhone((prev) => prev + digit);
  }

  function handleOtpDigit(digit: string) {
    touch();
    if (otp.length < 4) setOtp((prev) => prev + digit);
  }

  // ─── Auth handlers ─────────────────────────────────────────────────────

  async function handleSendOtp() {
    if (phone.length < 10) return;
    setLoading(true);
    setErrorMsg("");
    try {
      const res = await api.customerLogin(phone);
      if (res.error) {
        setErrorMsg(res.error);
      } else {
        setPhase("otp");
      }
    } catch {
      setErrorMsg("Network error — please try again");
    }
    setLoading(false);
  }

  async function handleVerifyOtp() {
    if (otp.length !== 4) return;
    setLoading(true);
    setErrorMsg("");
    try {
      const res = await api.customerVerifyOtp(phone, otp);
      if (res.error) {
        setErrorMsg(res.error);
        setOtp("");
      } else if (res.token) {
        setAuthToken(res.token);
        setDriverName(res.driver_name || "Racer");
        setDriverId(res.driver_id || "");
        // Set driver in wizard state
        wizard.setField("selectedDriver", {
          id: res.driver_id || "",
          name: res.driver_name || "Racer",
          total_laps: 0,
          total_time_ms: 0,
        });
        setPhase("wizard");
      }
    } catch {
      setErrorMsg("Network error — please try again");
      setOtp("");
    }
    setLoading(false);
  }

  // Auto-submit OTP when 4 digits entered
  useEffect(() => {
    if (otp.length === 4 && phase === "otp") {
      handleVerifyOtp();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [otp]);

  // ─── Wizard step handlers ─────────────────────────────────────────────

  function handleSelectTier(tier: PricingTier) {
    wizard.setField("selectedTier", tier);
    wizard.goNext();
  }

  function handleSelectGame(gameId: string) {
    wizard.setField("selectedGame", gameId);
    wizard.goNext();
  }

  function handleSelectPlayerMode(mode: "single" | "multi") {
    wizard.setField("playerMode", mode);
    wizard.goNext();
  }

  function handleSelectSessionType(type: "practice" | "qualification" | "race") {
    wizard.setField("sessionType", type);
    wizard.goNext();
  }

  function handleSelectExperience(exp: KioskExperience) {
    wizard.setField("selectedExperience", exp);
    wizard.goToStep("driving_settings");
  }

  function handleSelectTrack(track: CatalogItem) {
    wizard.setField("selectedTrack", track);
    wizard.goNext();
  }

  function handleSelectCar(car: CatalogItem) {
    wizard.setField("selectedCar", car);
    wizard.goNext();
  }

  // ─── Booking handler ──────────────────────────────────────────────────

  async function handleBook() {
    if (!wizard.state.selectedTier) return;
    setPhase("booking");
    setErrorMsg("");

    const ws = wizard.state;
    const bookingData: {
      pricing_tier_id: string;
      experience_id?: string;
      custom?: Record<string, unknown>;
    } = {
      pricing_tier_id: ws.selectedTier!.id,
    };

    if (ws.experienceMode === "preset" && ws.selectedExperience) {
      bookingData.experience_id = ws.selectedExperience.id;
    } else {
      // Build custom config from wizard state (same shape as buildLaunchArgs)
      bookingData.custom = JSON.parse(wizard.buildLaunchArgs());
    }

    try {
      const res = await api.customerBook(authToken, bookingData);
      if (res.error) {
        setErrorMsg(res.error);
        setPhase("error");
        return;
      }
      setResultPin(res.pin || "");
      setResultPodNumber(res.pod_number || 0);
      setResultAllocatedSeconds(res.allocated_seconds || 0);
      setPhase("success");
    } catch {
      setErrorMsg("Network error — please try again");
      setPhase("error");
    }
  }

  // ─── Navigation helpers ────────────────────────────────────────────────

  function handleWizardBack() {
    const ws = wizard.state;
    // If on the first wizard step (select_plan), go back to phone auth
    if (ws.currentStep === "select_plan") {
      setPhase("phone");
      setPhone("");
      setOtp("");
      setAuthToken("");
      wizard.reset();
      return;
    }
    wizard.goBack();
  }

  function handleCancel() {
    router.push("/");
  }

  const ws = wizard.state;
  const step = ws.currentStep;

  // ═══════════════════════════════════════════════════════════════════════
  // PHASE: PHONE NUMBER ENTRY
  // ═══════════════════════════════════════════════════════════════════════
  if (phase === "phone") {
    return (
      <div className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col items-center justify-center px-8" onClick={touch}>
        {/* Header */}
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-white mb-2">Book a Session</h1>
          <p className="text-rp-grey">Enter your registered phone number</p>
        </div>

        {/* Phone display */}
        <div className="mb-8 text-center">
          <p className="text-4xl font-bold text-white font-[family-name:var(--font-mono-jb)] tracking-wider">
            {phone || <span className="text-rp-grey/50">Phone number</span>}
          </p>
          {errorMsg && <p className="text-red-400 text-sm mt-2">{errorMsg}</p>}
        </div>

        {/* Numpad */}
        <div className="grid grid-cols-3 gap-3 w-full max-w-sm">
          {["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((digit) => (
            <button
              key={digit}
              onClick={() => handlePhoneDigit(digit)}
              className="h-20 rounded-xl bg-rp-surface border border-rp-border text-3xl font-bold text-white hover:bg-rp-red/10 hover:border-rp-red/50 active:bg-rp-red/20 transition-colors"
            >
              {digit}
            </button>
          ))}
          <button
            onClick={() => { touch(); setPhone(""); }}
            className="h-20 rounded-xl bg-rp-surface border border-rp-border text-sm font-semibold text-rp-grey hover:text-white hover:border-rp-red/50 transition-colors"
          >
            Clear
          </button>
          <button
            onClick={() => handlePhoneDigit("0")}
            className="h-20 rounded-xl bg-rp-surface border border-rp-border text-3xl font-bold text-white hover:bg-rp-red/10 hover:border-rp-red/50 active:bg-rp-red/20 transition-colors"
          >
            0
          </button>
          <button
            onClick={() => { touch(); setPhone((prev) => prev.slice(0, -1)); }}
            className="h-20 rounded-xl bg-rp-surface border border-rp-border flex items-center justify-center text-rp-grey hover:text-white hover:border-rp-red/50 transition-colors"
          >
            <svg className="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2M3 12l6.414-6.414A2 2 0 0110.828 5H21a2 2 0 012 2v10a2 2 0 01-2 2H10.828a2 2 0 01-1.414-.586L3 12z" />
            </svg>
          </button>
        </div>

        {/* Send OTP button */}
        <button
          onClick={handleSendOtp}
          disabled={phone.length < 10 || loading}
          className="mt-6 w-full max-w-sm py-4 bg-rp-red hover:bg-rp-red-hover disabled:opacity-40 disabled:cursor-not-allowed text-white font-bold text-lg rounded-xl transition-colors"
        >
          {loading ? "Sending..." : "Send OTP"}
        </button>

        {/* Cancel */}
        <button
          onClick={handleCancel}
          className="mt-4 text-rp-grey text-sm hover:text-white transition-colors"
        >
          Cancel
        </button>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════
  // PHASE: OTP ENTRY
  // ═══════════════════════════════════════════════════════════════════════
  if (phase === "otp") {
    return (
      <div className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col items-center justify-center px-8" onClick={touch}>
        {/* Header */}
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-white mb-2">Enter OTP</h1>
          <p className="text-rp-grey">Sent to {phone}</p>
        </div>

        {/* OTP display boxes */}
        <div className="flex gap-4 mb-10">
          {[0, 1, 2, 3].map((i) => (
            <div
              key={i}
              className={`w-20 h-24 rounded-xl border-2 flex items-center justify-center transition-all ${
                i < otp.length
                  ? "border-rp-red bg-rp-red/10"
                  : i === otp.length
                  ? "border-rp-red/50 bg-rp-surface"
                  : "border-rp-border bg-rp-surface"
              }`}
            >
              <span className="text-5xl font-bold text-white font-[family-name:var(--font-mono-jb)]">
                {otp[i] || ""}
              </span>
            </div>
          ))}
        </div>

        {errorMsg && <p className="text-red-400 text-sm mb-4">{errorMsg}</p>}

        {/* Numpad */}
        <div className="grid grid-cols-3 gap-3 w-full max-w-sm">
          {["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((digit) => (
            <button
              key={digit}
              onClick={() => handleOtpDigit(digit)}
              className="h-20 rounded-xl bg-rp-surface border border-rp-border text-3xl font-bold text-white hover:bg-rp-red/10 hover:border-rp-red/50 active:bg-rp-red/20 transition-colors"
            >
              {digit}
            </button>
          ))}
          <button
            onClick={() => { touch(); setOtp(""); }}
            className="h-20 rounded-xl bg-rp-surface border border-rp-border text-sm font-semibold text-rp-grey hover:text-white hover:border-rp-red/50 transition-colors"
          >
            Clear
          </button>
          <button
            onClick={() => handleOtpDigit("0")}
            className="h-20 rounded-xl bg-rp-surface border border-rp-border text-3xl font-bold text-white hover:bg-rp-red/10 hover:border-rp-red/50 active:bg-rp-red/20 transition-colors"
          >
            0
          </button>
          <button
            onClick={() => { touch(); setOtp((prev) => prev.slice(0, -1)); }}
            className="h-20 rounded-xl bg-rp-surface border border-rp-border flex items-center justify-center text-rp-grey hover:text-white hover:border-rp-red/50 transition-colors"
          >
            <svg className="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2M3 12l6.414-6.414A2 2 0 0110.828 5H21a2 2 0 012 2v10a2 2 0 01-2 2H10.828a2 2 0 01-1.414-.586L3 12z" />
            </svg>
          </button>
        </div>

        {loading && (
          <div className="mt-6 flex items-center gap-2">
            <div className="w-5 h-5 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
            <span className="text-rp-grey text-sm">Verifying...</span>
          </div>
        )}

        {/* Back */}
        <button
          onClick={() => { setPhase("phone"); setOtp(""); setErrorMsg(""); }}
          className="mt-6 text-rp-grey text-sm hover:text-white transition-colors"
        >
          Back
        </button>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════
  // PHASE: BOOKING IN PROGRESS — Spinner
  // ═══════════════════════════════════════════════════════════════════════
  if (phase === "booking") {
    return (
      <div className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col items-center justify-center">
        <div className="w-16 h-16 border-4 border-rp-red border-t-transparent rounded-full animate-spin mb-6" />
        <p className="text-xl text-white font-semibold">Booking your session...</p>
        <p className="text-rp-grey text-sm mt-2">Finding the best rig for you</p>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════
  // PHASE: SUCCESS — PIN + Pod Display
  // ═══════════════════════════════════════════════════════════════════════
  if (phase === "success") {
    const minutes = Math.floor(resultAllocatedSeconds / 60);

    return (
      <div className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col items-center justify-center gap-8">
        {/* Checkmark */}
        <div className="w-20 h-20 rounded-full bg-rp-green/20 flex items-center justify-center">
          <svg className="w-10 h-10 text-rp-green" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
          </svg>
        </div>

        <div className="text-center">
          <h1 className="text-3xl font-bold text-white">
            You&apos;re booked, {driverName}!
          </h1>
          <p className="text-rp-grey text-lg mt-2">Head to your assigned rig</p>
        </div>

        {/* Pod number */}
        <div className="bg-rp-surface border-2 border-rp-red rounded-2xl p-8 text-center glow-active">
          <p className="text-sm text-rp-grey uppercase tracking-wider mb-2">Go to Rig</p>
          <p className="text-8xl font-bold text-white font-[family-name:var(--font-display)]">
            {resultPodNumber}
          </p>
        </div>

        {/* PIN display */}
        <div className="text-center">
          <p className="text-rp-grey text-sm mb-1">Your PIN</p>
          <div className="flex gap-3 justify-center">
            {resultPin.split("").map((digit, i) => (
              <div
                key={i}
                className="w-16 h-20 rounded-xl border-2 border-rp-red bg-rp-red/10 flex items-center justify-center"
              >
                <span className="text-4xl font-bold text-white font-[family-name:var(--font-mono-jb)]">
                  {digit}
                </span>
              </div>
            ))}
          </div>
        </div>

        {/* Session info */}
        <div className="text-center space-y-1">
          {ws.selectedTier && (
            <p className="text-rp-grey text-sm">{ws.selectedTier.name}</p>
          )}
          {minutes > 0 && (
            <p className="text-white font-semibold">{minutes} minutes</p>
          )}
        </div>

        {/* Auto-return notice */}
        <p className="text-rp-grey text-sm mt-4">
          This screen will reset automatically
        </p>

        <button
          onClick={() => router.push("/")}
          className="px-8 py-3 border border-rp-border rounded-lg text-rp-grey hover:text-white hover:border-rp-red transition-colors"
        >
          Done
        </button>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════
  // PHASE: ERROR
  // ═══════════════════════════════════════════════════════════════════════
  if (phase === "error") {
    return (
      <div className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col items-center justify-center gap-6">
        <div className="w-20 h-20 rounded-full bg-red-900/30 flex items-center justify-center">
          <svg className="w-10 h-10 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </div>

        <div className="text-center">
          <h1 className="text-3xl font-bold text-white mb-2">Booking Failed</h1>
          <p className="text-rp-grey">{errorMsg || "Something went wrong — please try again"}</p>
        </div>

        <button
          onClick={() => { setPhase("wizard"); wizard.goToStep("review"); }}
          className="px-8 py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-lg transition-colors"
        >
          Try Again
        </button>

        <button
          onClick={handleCancel}
          className="text-rp-grey text-sm hover:text-white transition-colors"
        >
          Back to Home
        </button>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════
  // PHASE: WIZARD — Game Configuration Steps
  // ═══════════════════════════════════════════════════════════════════════
  return (
    <div className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col">
      {/* Top Bar */}
      <div className="px-6 py-4 border-b border-rp-border bg-rp-card/50 shrink-0">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-2 text-xs text-rp-grey">
              <span>Welcome, {driverName}</span>
              {ws.selectedTier && (
                <>
                  <span>&middot;</span>
                  <span>{ws.selectedTier.name}</span>
                </>
              )}
            </div>
            <h2 className="text-xl font-bold text-white mt-1">
              {STEP_TITLES[step] || step}
            </h2>
          </div>
          <button
            onClick={handleCancel}
            className="px-4 py-2 border border-rp-border rounded-lg text-sm text-rp-grey hover:text-white hover:border-rp-red transition-colors"
          >
            Cancel
          </button>
        </div>
      </div>

      {/* Step Content */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-2xl mx-auto">

          {/* ─── SELECT PLAN ──────────────────────────────────────── */}
          {step === "select_plan" && (
            <div className="space-y-3">
              {tiers.map((tier) => (
                <button
                  key={tier.id}
                  onClick={() => handleSelectTier(tier)}
                  className="w-full flex items-center justify-between px-6 py-5 bg-rp-surface border-2 border-rp-border rounded-xl hover:border-rp-red/50 transition-all"
                >
                  <div className="text-left">
                    <p className="text-lg font-bold text-white">{tier.name}</p>
                    <p className="text-sm text-rp-grey">{tier.duration_minutes} minutes</p>
                  </div>
                  <span className="text-lg font-bold text-rp-red">
                    {tier.is_trial ? "Free Trial" : `${(tier.price_paise / 100).toFixed(0)} credits`}
                  </span>
                </button>
              ))}
            </div>
          )}

          {/* ─── SELECT GAME ──────────────────────────────────────── */}
          {step === "select_game" && (
            <div className="grid grid-cols-2 gap-4">
              {GAMES.map((g) => (
                <button
                  key={g.id}
                  disabled={!g.enabled}
                  onClick={() => handleSelectGame(g.id)}
                  className={`p-8 rounded-xl border-2 text-center transition-all ${
                    g.enabled
                      ? "border-rp-border bg-rp-surface hover:border-rp-red hover:bg-rp-red/10 cursor-pointer"
                      : "border-rp-border/50 bg-rp-surface/50 opacity-40 cursor-not-allowed"
                  }`}
                >
                  <p className="text-xl font-bold text-white">{g.name}</p>
                  {!g.enabled && <p className="text-sm text-rp-grey mt-1">Coming Soon</p>}
                </button>
              ))}
            </div>
          )}

          {/* ─── PLAYER MODE ──────────────────────────────────────── */}
          {step === "player_mode" && (
            <div className="grid grid-cols-2 gap-6">
              <button
                onClick={() => handleSelectPlayerMode("single")}
                className="p-8 rounded-xl border-2 border-rp-border bg-rp-surface hover:border-rp-red hover:bg-rp-red/10 transition-all text-center"
              >
                <svg className="w-12 h-12 mx-auto mb-3 text-rp-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                </svg>
                <p className="text-xl font-bold text-white">Singleplayer</p>
                <p className="text-sm text-rp-grey mt-1">Practice &amp; hot laps</p>
              </button>
              <button
                onClick={() => handleSelectPlayerMode("multi")}
                className="p-8 rounded-xl border-2 border-rp-border bg-rp-surface hover:border-rp-red hover:bg-rp-red/10 transition-all text-center"
              >
                <svg className="w-12 h-12 mx-auto mb-3 text-rp-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z" />
                </svg>
                <p className="text-xl font-bold text-white">Multiplayer</p>
                <p className="text-sm text-rp-grey mt-1">Race with others</p>
              </button>
            </div>
          )}

          {/* ─── SESSION TYPE ─────────────────────────────────────── */}
          {step === "session_type" && (
            <div className="space-y-4">
              {([
                { type: "practice" as const, label: "Practice", desc: "Free driving with no timer or AI pressure", icon: "M13 10V3L4 14h7v7l9-11h-7z" },
                { type: "qualification" as const, label: "Qualification", desc: "Timed laps — set the fastest time", icon: "M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" },
                { type: "race" as const, label: "Race", desc: "Full race with grid start and laps", icon: "M3 21v-4m0 0V5a2 2 0 012-2h6.5l1 1H21l-3 6 3 6h-8.5l-1-1H5a2 2 0 00-2 2zm9-13.5V9" },
              ]).map(({ type, label, desc, icon }) => (
                <button
                  key={type}
                  onClick={() => handleSelectSessionType(type)}
                  className={`w-full flex items-center gap-5 px-6 py-5 rounded-xl border-2 transition-all text-left ${
                    ws.sessionType === type
                      ? "border-rp-red bg-rp-red/10"
                      : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                  }`}
                >
                  <svg className="w-10 h-10 text-rp-red shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d={icon} />
                  </svg>
                  <div>
                    <p className="text-lg font-bold text-white">{label}</p>
                    <p className="text-sm text-rp-grey">{desc}</p>
                  </div>
                </button>
              ))}
            </div>
          )}

          {/* ─── AI CONFIG ────────────────────────────────────────── */}
          {step === "ai_config" && (
            <div className="space-y-8">
              <div className="flex items-center justify-between px-6 py-4 bg-rp-surface border border-rp-border rounded-xl">
                <div>
                  <p className="text-lg font-semibold text-white">AI Opponents</p>
                  <p className="text-sm text-rp-grey">Race against computer-controlled drivers</p>
                </div>
                <button
                  onClick={() => wizard.setField("aiEnabled", !ws.aiEnabled)}
                  className={`w-14 h-7 rounded-full transition-colors ${ws.aiEnabled ? "bg-rp-red" : "bg-zinc-700"}`}
                >
                  <div className={`w-6 h-6 bg-white rounded-full transition-transform mx-0.5 ${ws.aiEnabled ? "translate-x-7" : ""}`} />
                </button>
              </div>

              {ws.aiEnabled && (
                <>
                  <div>
                    <h4 className="text-sm font-semibold text-white mb-3">AI Difficulty</h4>
                    <div className="grid grid-cols-3 gap-4">
                      {(["easy", "medium", "hard"] as const).map((level) => (
                        <button
                          key={level}
                          onClick={() => wizard.setField("aiDifficulty", level)}
                          className={`p-5 rounded-xl border-2 text-center transition-all ${
                            ws.aiDifficulty === level
                              ? "border-rp-red bg-rp-red/10"
                              : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                          }`}
                        >
                          <p className="text-base font-bold text-white capitalize">{level}</p>
                        </button>
                      ))}
                    </div>
                  </div>

                  <div>
                    <h4 className="text-sm font-semibold text-white mb-3">
                      Number of AI: <span className="text-rp-red">{ws.aiCount}</span>
                    </h4>
                    <input
                      type="range"
                      min={1}
                      max={20}
                      value={ws.aiCount}
                      onChange={(e) => wizard.setField("aiCount", parseInt(e.target.value))}
                      className="w-full accent-rp-red"
                    />
                    <div className="flex justify-between text-xs text-rp-grey mt-1">
                      <span>1</span>
                      <span>20</span>
                    </div>
                  </div>
                </>
              )}

              <button
                onClick={() => wizard.goNext()}
                className="w-full py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-xl transition-colors"
              >
                Continue
              </button>
            </div>
          )}

          {/* ─── MULTIPLAYER LOBBY ────────────────────────────────── */}
          {step === "multiplayer_lobby" && (
            <div className="space-y-6">
              <div className="grid grid-cols-2 gap-4">
                <button
                  onClick={() => wizard.setField("multiplayerMode", "create")}
                  className={`p-6 rounded-xl border-2 text-center transition-all ${
                    ws.multiplayerMode === "create"
                      ? "border-rp-red bg-rp-red/10"
                      : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                  }`}
                >
                  <svg className="w-10 h-10 mx-auto mb-2 text-rp-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6v6m0 0v6m0-6h6m-6 0H6" />
                  </svg>
                  <p className="text-base font-bold text-white">Create Server</p>
                  <p className="text-xs text-rp-grey mt-1">Host a race lobby</p>
                </button>
                <button
                  onClick={() => wizard.setField("multiplayerMode", "join")}
                  className={`p-6 rounded-xl border-2 text-center transition-all ${
                    ws.multiplayerMode === "join"
                      ? "border-rp-red bg-rp-red/10"
                      : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                  }`}
                >
                  <svg className="w-10 h-10 mx-auto mb-2 text-rp-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 16l-4-4m0 0l4-4m-4 4h14m-5 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h7a3 3 0 013 3v1" />
                  </svg>
                  <p className="text-base font-bold text-white">Join Server</p>
                  <p className="text-xs text-rp-grey mt-1">Join an existing lobby</p>
                </button>
              </div>

              {ws.multiplayerMode === "join" && (
                <div className="space-y-3">
                  <div>
                    <label className="text-xs text-rp-grey uppercase tracking-wider block mb-1">Server IP</label>
                    <input type="text" placeholder="192.168.31.51" value={ws.serverIp}
                      onChange={(e) => wizard.setField("serverIp", e.target.value)}
                      className="w-full px-4 py-3 bg-rp-surface border border-rp-border rounded-lg text-white focus:outline-none focus:border-rp-red" />
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="text-xs text-rp-grey uppercase tracking-wider block mb-1">Port</label>
                      <input type="text" placeholder="9600" value={ws.serverPort}
                        onChange={(e) => wizard.setField("serverPort", e.target.value)}
                        className="w-full px-4 py-3 bg-rp-surface border border-rp-border rounded-lg text-white focus:outline-none focus:border-rp-red" />
                    </div>
                    <div>
                      <label className="text-xs text-rp-grey uppercase tracking-wider block mb-1">HTTP Port</label>
                      <input type="text" placeholder="8081" value={ws.serverHttpPort}
                        onChange={(e) => wizard.setField("serverHttpPort", e.target.value)}
                        className="w-full px-4 py-3 bg-rp-surface border border-rp-border rounded-lg text-white focus:outline-none focus:border-rp-red" />
                    </div>
                  </div>
                  <div>
                    <label className="text-xs text-rp-grey uppercase tracking-wider block mb-1">Password (optional)</label>
                    <input type="text" placeholder="Server password" value={ws.serverPassword}
                      onChange={(e) => wizard.setField("serverPassword", e.target.value)}
                      className="w-full px-4 py-3 bg-rp-surface border border-rp-border rounded-lg text-white focus:outline-none focus:border-rp-red" />
                  </div>
                </div>
              )}

              {ws.multiplayerMode === "create" && (
                <div className="bg-rp-surface border border-rp-border rounded-xl p-5">
                  <p className="text-sm text-rp-grey">
                    A dedicated server will be started on Racing-Point-Server.
                    Other pods can join using the server details shown after creation.
                  </p>
                </div>
              )}

              {ws.multiplayerMode && (
                <button
                  onClick={() => wizard.goNext()}
                  disabled={ws.multiplayerMode === "join" && !ws.serverIp}
                  className="w-full py-4 bg-rp-red hover:bg-rp-red-hover disabled:opacity-40 text-white font-bold text-lg rounded-xl transition-colors"
                >
                  Continue
                </button>
              )}
            </div>
          )}

          {/* ─── SELECT EXPERIENCE (preset) ───────────────────────── */}
          {step === "select_experience" && (
            <div className="space-y-4">
              <div className="flex gap-3">
                <button
                  onClick={() => wizard.setField("experienceMode", "preset")}
                  className={`flex-1 py-3 rounded-xl text-sm font-medium transition-colors ${
                    ws.experienceMode === "preset"
                      ? "bg-rp-red text-white"
                      : "bg-rp-surface border border-rp-border text-rp-grey hover:text-white"
                  }`}
                >
                  Preset Experiences
                </button>
                <button
                  onClick={() => {
                    wizard.setField("experienceMode", "custom");
                    wizard.goToStep("select_track");
                  }}
                  className="flex-1 py-3 rounded-xl text-sm font-medium transition-colors bg-rp-surface border border-rp-border text-rp-grey hover:text-white"
                >
                  Custom (Track + Car)
                </button>
              </div>

              <div className="space-y-3 max-h-[60vh] overflow-y-auto">
                {experiences.length === 0 ? (
                  <p className="text-sm text-rp-grey text-center py-8">No experiences configured</p>
                ) : (
                  experiences.map((exp) => (
                    <button
                      key={exp.id}
                      onClick={() => handleSelectExperience(exp)}
                      className="w-full flex items-center gap-4 px-5 py-4 bg-rp-surface border-2 border-rp-border rounded-xl hover:border-rp-red/50 transition-all text-left"
                    >
                      {exp.car_class && (
                        <span className={`w-9 h-9 flex items-center justify-center rounded-lg text-sm font-bold ${CLASS_COLORS[exp.car_class] || "bg-zinc-600 text-white"}`}>
                          {exp.car_class}
                        </span>
                      )}
                      <div className="flex-1 min-w-0">
                        <p className="text-base font-semibold text-white truncate">{exp.name}</p>
                        <p className="text-sm text-rp-grey truncate">{exp.track} &middot; {exp.car}</p>
                      </div>
                      <div className="text-right shrink-0">
                        <p className="text-xs text-rp-grey">{GAME_LABELS[exp.game] || exp.game}</p>
                      </div>
                    </button>
                  ))
                )}
              </div>
            </div>
          )}

          {/* ─── SELECT TRACK ─────────────────────────────────────── */}
          {step === "select_track" && (
            <div className="space-y-4">
              <input
                type="text"
                value={trackSearch}
                onChange={(e) => setTrackSearch(e.target.value)}
                placeholder="Search tracks..."
                className="w-full px-4 py-3 bg-rp-surface border border-rp-border rounded-xl text-white placeholder:text-rp-grey focus:outline-none focus:border-rp-red"
              />
              <div className="flex gap-2 overflow-x-auto pb-2">
                {trackCategories.map((cat) => (
                  <button
                    key={cat}
                    onClick={() => setTrackCategory(cat)}
                    className={`px-4 py-2 rounded-full text-xs font-medium whitespace-nowrap transition-colors ${
                      trackCategory === cat
                        ? "bg-rp-red text-white"
                        : "bg-rp-surface border border-rp-border text-rp-grey hover:text-white"
                    }`}
                  >
                    {cat}
                  </button>
                ))}
              </div>
              <div className="grid grid-cols-2 gap-3 max-h-[55vh] overflow-y-auto">
                {filteredTracks.map((t) => (
                  <button
                    key={t.id}
                    onClick={() => handleSelectTrack(t)}
                    className={`p-4 rounded-xl border-2 text-left transition-all ${
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
            <div className="space-y-4">
              <input
                type="text"
                value={carSearch}
                onChange={(e) => setCarSearch(e.target.value)}
                placeholder="Search cars..."
                className="w-full px-4 py-3 bg-rp-surface border border-rp-border rounded-xl text-white placeholder:text-rp-grey focus:outline-none focus:border-rp-red"
              />
              <div className="flex gap-2 overflow-x-auto pb-2">
                {carCategories.map((cat) => (
                  <button
                    key={cat}
                    onClick={() => setCarCategory(cat)}
                    className={`px-4 py-2 rounded-full text-xs font-medium whitespace-nowrap transition-colors ${
                      carCategory === cat
                        ? "bg-rp-red text-white"
                        : "bg-rp-surface border border-rp-border text-rp-grey hover:text-white"
                    }`}
                  >
                    {cat}
                  </button>
                ))}
              </div>
              <div className="grid grid-cols-2 gap-3 max-h-[55vh] overflow-y-auto">
                {filteredCars.map((c) => (
                  <button
                    key={c.id}
                    onClick={() => handleSelectCar(c)}
                    className={`p-4 rounded-xl border-2 text-left transition-all ${
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
            <div className="space-y-8">
              <div>
                <h4 className="text-base font-semibold text-white mb-3">Difficulty</h4>
                <div className="grid grid-cols-3 gap-4">
                  {Object.entries(DIFFICULTY_PRESETS).map(([key, preset]) => (
                    <button
                      key={key}
                      onClick={() => wizard.setField("drivingDifficulty", key)}
                      className={`p-5 rounded-xl border-2 text-center transition-all ${
                        ws.drivingDifficulty === key
                          ? "border-rp-red bg-rp-red/10"
                          : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                      }`}
                    >
                      <p className="text-base font-bold text-white">{preset.label}</p>
                      <p className="text-xs text-rp-grey mt-1">{preset.desc}</p>
                    </button>
                  ))}
                </div>
              </div>

              <div>
                <h4 className="text-base font-semibold text-white mb-3">Transmission</h4>
                <div className="grid grid-cols-2 gap-4">
                  {([
                    { key: "auto", label: "Automatic", desc: "Auto gear shifts" },
                    { key: "manual", label: "Manual", desc: "Paddle shifters" },
                  ] as const).map(({ key, label, desc }) => (
                    <button
                      key={key}
                      onClick={() => wizard.setField("transmission", key)}
                      className={`p-5 rounded-xl border-2 text-center transition-all ${
                        ws.transmission === key
                          ? "border-rp-red bg-rp-red/10"
                          : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                      }`}
                    >
                      <p className="text-base font-bold text-white">{label}</p>
                      <p className="text-xs text-rp-grey mt-1">{desc}</p>
                    </button>
                  ))}
                </div>
              </div>

              <div>
                <h4 className="text-base font-semibold text-white mb-3">Force Feedback</h4>
                <div className="grid grid-cols-3 gap-4">
                  {([
                    { key: "light", label: "Light", desc: "Casual / kids" },
                    { key: "medium", label: "Medium", desc: "Balanced default" },
                    { key: "strong", label: "Strong", desc: "Full force" },
                  ] as const).map(({ key, label, desc }) => (
                    <button
                      key={key}
                      onClick={() => wizard.setField("ffb", key)}
                      className={`p-5 rounded-xl border-2 text-center transition-all ${
                        ws.ffb === key
                          ? "border-rp-red bg-rp-red/10"
                          : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                      }`}
                    >
                      <p className="text-base font-bold text-white">{label}</p>
                      <p className="text-xs text-rp-grey mt-1">{desc}</p>
                    </button>
                  ))}
                </div>
              </div>

              <button
                onClick={() => wizard.goNext()}
                className="w-full py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-xl transition-colors"
              >
                Review
              </button>
            </div>
          )}

          {/* ─── REVIEW & BOOK ────────────────────────────────────── */}
          {step === "review" && (
            <div className="space-y-6">
              <div className="bg-rp-surface border-2 border-rp-border rounded-xl p-6 space-y-4">
                <ReviewRow label="Driver" value={driverName} />
                <ReviewRow label="Plan" value={ws.selectedTier?.name || ""} />
                <ReviewRow label="Game" value={GAME_LABELS[ws.selectedGame] || ws.selectedGame} />
                <ReviewRow label="Mode" value={ws.playerMode === "multi" ? "Multiplayer" : "Singleplayer"} />
                <ReviewRow label="Session" value={ws.sessionType.charAt(0).toUpperCase() + ws.sessionType.slice(1)} />
                {ws.aiEnabled && (
                  <ReviewRow label="AI Opponents" value={`${ws.aiCount} (${ws.aiDifficulty})`} />
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
                onClick={handleBook}
                className="w-full py-5 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-xl rounded-xl transition-colors"
              >
                BOOK SESSION
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Footer with Back button */}
      <div className="px-6 py-4 border-t border-rp-border shrink-0">
        <button
          onClick={handleWizardBack}
          className="px-6 py-3 border border-rp-border rounded-xl text-sm text-rp-grey hover:text-white hover:border-rp-red transition-colors"
        >
          Back
        </button>
      </div>
    </div>
  );
}

// ─── Helper Components ──────────────────────────────────────────────────────

function ReviewRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between">
      <span className="text-rp-grey text-sm">{label}</span>
      <span className="text-white font-semibold text-sm">{value}</span>
    </div>
  );
}
