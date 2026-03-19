"use client";

import { Suspense, useState, useEffect, useCallback, useMemo } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { api } from "@/lib/api";
import { useSetupWizard } from "@/hooks/useSetupWizard";
import type { PricingTier, AcCatalog, CatalogItem, KioskExperience, SessionType, KioskMultiplayerAssignment } from "@/lib/types";
import { DIFFICULTY_PRESETS, GAMES, GAME_LABELS, CLASS_COLORS } from "@/lib/constants";

// ─── Phase Definitions ──────────────────────────────────────────────────────

type Phase = "phone" | "otp" | "wizard" | "booking" | "success" | "error";

// ─── Constants ──────────────────────────────────────────────────────────────

const AUTO_RETURN_MS = 30_000;
const INACTIVITY_MS = 120_000;

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

// ─── Suspense Wrapper (required for useSearchParams in Next.js) ─────────────

export default function BookingPage() {
  return (
    <Suspense fallback={<div className="h-screen w-screen bg-rp-black" />}>
      <BookingPageInner />
    </Suspense>
  );
}

// ─── Main Booking Page ──────────────────────────────────────────────────────

function BookingPageInner() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const wizard = useSetupWizard();

  // ─── Staff mode detection ────────────────────────────────────────────
  const isStaffMode = searchParams.get("staff") === "true";
  const staffPodId = searchParams.get("pod") || "";

  // ─── Auth state ────────────────────────────────────────────────────────
  const [phase, setPhase] = useState<Phase>(isStaffMode ? "phone" : "phone");
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

  // ─── Multiplayer state ──────────────────────────────────────────────
  const [podCount, setPodCount] = useState(2);
  const [multiAssignments, setMultiAssignments] = useState<KioskMultiplayerAssignment[]>([]);
  const [multiExperienceName, setMultiExperienceName] = useState("");

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

  const returnPath = isStaffMode ? "/control" : "/";

  // Success screen auto-returns
  useEffect(() => {
    if (phase !== "success") return;
    const timer = setTimeout(() => router.push(returnPath), AUTO_RETURN_MS);
    return () => clearTimeout(timer);
  }, [phase, router, returnPath]);

  // Inactivity auto-returns during phone/otp entry
  useEffect(() => {
    if (phase !== "phone" && phase !== "otp") return;
    const interval = setInterval(() => {
      if (Date.now() - lastActivity > INACTIVITY_MS) {
        router.push(returnPath);
      }
    }, 5000);
    return () => clearInterval(interval);
  }, [phase, lastActivity, router, returnPath]);

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

  function handleSelectSessionType(type: SessionType) {
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

  // ─── Multiplayer booking handler ──────────────────────────────────────

  async function handleBookMultiplayer() {
    if (!wizard.state.selectedTier) return;
    setPhase("booking");
    setErrorMsg("");

    const ws = wizard.state;
    const bookingData: {
      pricing_tier_id: string;
      pod_count: number;
      experience_id?: string;
      custom?: Record<string, unknown>;
    } = {
      pricing_tier_id: ws.selectedTier!.id,
      pod_count: podCount,
    };

    if (ws.experienceMode === "preset" && ws.selectedExperience) {
      bookingData.experience_id = ws.selectedExperience.id;
    } else {
      bookingData.custom = JSON.parse(wizard.buildLaunchArgs());
    }

    try {
      const res = await api.kioskBookMultiplayer(authToken, bookingData);
      if (res.error) {
        setErrorMsg(res.error);
        setPhase("error");
        return;
      }
      setMultiAssignments(res.assignments || []);
      setMultiExperienceName(res.experience_name || "");
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
    // If on the first wizard step (select_plan), go back to phone auth or control
    if (ws.currentStep === "select_plan") {
      if (isStaffMode && authToken === "staff-walkin") {
        // Walk-in staff: go back to phone screen (where they can choose walk-in again or enter phone)
        setPhase("phone");
        setAuthToken("");
        setDriverName("");
        setDriverId("");
        wizard.reset();
        setPodCount(2);
        setMultiAssignments([]);
        setMultiExperienceName("");
      } else {
        setPhase("phone");
        setPhone("");
        setOtp("");
        setAuthToken("");
        wizard.reset();
        setPodCount(2);
        setMultiAssignments([]);
        setMultiExperienceName("");
      }
      return;
    }
    wizard.goBack();
  }

  function handleCancel() {
    router.push(returnPath);
  }

  // ─── Staff walk-in (anonymous) handler ──────────────────────────────
  function handleStaffWalkIn() {
    setDriverName("Walk-in");
    setDriverId("");
    setAuthToken("staff-walkin");
    wizard.setField("selectedDriver", {
      id: "walkin",
      name: "Walk-in",
      total_laps: 0,
      total_time_ms: 0,
    });
    setPhase("wizard");
    // Log anonymous walk-in notification via debug incident (shows in activity feed)
    fetch(`${typeof window !== "undefined" ? `http://${window.location.hostname}:8080` : "http://localhost:8080"}/api/v1/debug/incidents`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        pod_id: staffPodId || null,
        description: "Staff walk-in booking initiated (no customer phone registered)",
        category: "billing",
      }),
    }).catch(() => {});
  }

  const ws = wizard.state;
  const step = ws.currentStep;

  // ═══════════════════════════════════════════════════════════════════════
  // PHASE: PHONE NUMBER ENTRY
  // ═══════════════════════════════════════════════════════════════════════
  if (phase === "phone") {
    return (
      <div data-testid="booking-phone-screen" className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col items-center justify-center px-8" onClick={touch}>
        {/* Header */}
        <div className="text-center mb-8">
          {isStaffMode && staffPodId && (
            <p className="text-rp-red text-sm font-semibold uppercase tracking-wider mb-2">
              Staff Mode &mdash; {staffPodId.replace("_", " ").toUpperCase()}
            </p>
          )}
          <h1 className="text-3xl font-bold text-white mb-2">Book a Session</h1>
          <p className="text-rp-grey">
            {isStaffMode ? "Enter customer phone or skip for walk-in" : "Enter your registered phone number"}
          </p>
        </div>

        {/* Phone display */}
        <div className="mb-8 text-center">
          <p data-testid="phone-display" className="text-4xl font-bold text-white font-[family-name:var(--font-mono-jb)] tracking-wider">
            {phone || <span className="text-rp-grey/50">Phone number</span>}
          </p>
          {errorMsg && <p className="text-red-400 text-sm mt-2">{errorMsg}</p>}
        </div>

        {/* Numpad */}
        <div className="grid grid-cols-3 gap-3 w-full max-w-sm">
          {["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((digit) => (
            <button
              key={digit}
              data-testid={`numpad-digit-${digit}`}
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
            data-testid="numpad-digit-0"
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
          data-testid="send-otp-btn"
          onClick={handleSendOtp}
          disabled={phone.length < 10 || loading}
          className="mt-6 w-full max-w-sm py-4 bg-rp-red hover:bg-rp-red-hover disabled:opacity-40 disabled:cursor-not-allowed text-white font-bold text-lg rounded-xl transition-colors"
        >
          {loading ? "Sending..." : "Send OTP"}
        </button>

        {/* Staff walk-in option */}
        {isStaffMode && (
          <button
            data-testid="walkin-btn"
            onClick={handleStaffWalkIn}
            className="mt-3 w-full max-w-sm py-4 bg-amber-600/20 border border-amber-600 text-amber-400 font-bold text-lg rounded-xl hover:bg-amber-600 hover:text-white transition-colors"
          >
            Walk-in (No Phone)
          </button>
        )}

        {/* Cancel */}
        <button
          data-testid="cancel-btn"
          onClick={handleCancel}
          className="mt-4 text-rp-grey text-sm hover:text-white transition-colors"
        >
          {isStaffMode ? "Back to Control" : "Cancel"}
        </button>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════
  // PHASE: OTP ENTRY
  // ═══════════════════════════════════════════════════════════════════════
  if (phase === "otp") {
    return (
      <div data-testid="booking-otp-screen" className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col items-center justify-center px-8" onClick={touch}>
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
              data-testid={`otp-digit-${digit}`}
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
            data-testid="otp-digit-0"
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
    const isMulti = multiAssignments.length > 0;

    return (
      <div data-testid="booking-success" className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col items-center justify-center gap-6 px-8">
        {/* Checkmark */}
        <div className="w-16 h-16 rounded-full bg-green-500/20 flex items-center justify-center">
          <svg className="w-8 h-8 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
          </svg>
        </div>

        <div className="text-center">
          <h1 className="text-3xl font-bold text-white">
            {isMulti ? "You\u0027re all set!" : `You\u0027re booked, ${driverName}!`}
          </h1>
          <p className="text-rp-grey text-lg mt-1">
            {isMulti ? "Head to your assigned rigs" : "Head to your assigned rig"}
          </p>
        </div>

        {isMulti ? (
          /* ─── Multiplayer: show all assignments ─── */
          <div className="w-full max-w-md space-y-3">
            {multiAssignments.map((a, i) => (
              <div
                key={i}
                className="bg-rp-surface border-2 border-rp-red rounded-xl p-4 flex items-center justify-between"
              >
                <div className="flex items-center gap-4">
                  <div className="text-center">
                    <p className="text-xs text-rp-grey uppercase tracking-wider">Rig</p>
                    <p className="text-4xl font-bold text-white font-[family-name:var(--font-display)]">
                      {a.pod_number}
                    </p>
                  </div>
                  <div className="w-px h-12 bg-rp-border" />
                  <div className="text-center">
                    <p className="text-xs text-rp-grey uppercase tracking-wider">PIN</p>
                    <div className="flex gap-1.5">
                      {a.pin.split("").map((digit, j) => (
                        <div
                          key={j}
                          className="w-10 h-12 rounded-lg border border-rp-red bg-rp-red/10 flex items-center justify-center"
                        >
                          <span className="text-xl font-bold text-white font-[family-name:var(--font-mono-jb)]">
                            {digit}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                </div>
                <div className="text-xs text-rp-grey uppercase">
                  {i === 0 ? "You" : `Friend ${i}`}
                </div>
              </div>
            ))}
          </div>
        ) : (
          /* ─── Single-player: existing pod + PIN display ─── */
          <>
            <div className="bg-rp-surface border-2 border-rp-red rounded-2xl p-8 text-center glow-active">
              <p className="text-sm text-rp-grey uppercase tracking-wider mb-2">Go to Rig</p>
              <p className="text-8xl font-bold text-white font-[family-name:var(--font-display)]">
                {resultPodNumber}
              </p>
            </div>

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
          </>
        )}

        {/* Session info */}
        <div className="text-center space-y-1">
          {isMulti && multiExperienceName && (
            <p className="text-rp-grey text-sm">{multiExperienceName}</p>
          )}
          {!isMulti && ws.selectedTier && (
            <p className="text-rp-grey text-sm">{ws.selectedTier.name}</p>
          )}
          {minutes > 0 && (
            <p className="text-white font-semibold">{minutes} minutes</p>
          )}
        </div>

        {/* Auto-return notice */}
        <p className="text-rp-grey text-sm mt-2">
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
      <div data-testid="booking-error" className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col items-center justify-center gap-6">
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
    <div data-testid="booking-wizard" className="h-screen w-screen overflow-hidden bg-rp-black flex flex-col">
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
            <h2 data-testid="wizard-step-title" className="text-xl font-bold text-white mt-1">
              {STEP_TITLES[step] || step}
            </h2>
          </div>
          <button
            data-testid="cancel-btn"
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
            <div data-testid="step-select-plan" className="space-y-3">
              {tiers.map((tier) => (
                <button
                  key={tier.id}
                  data-testid={`tier-option-${tier.id}`}
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
            <div data-testid="step-select-game" className="grid grid-cols-2 gap-4">
              {GAMES.map((g) => (
                <button
                  key={g.id}
                  data-testid={`game-option-${g.id}`}
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
            <div data-testid="step-player-mode" className="grid grid-cols-2 gap-6">
              <button
                data-testid="player-mode-single"
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
                data-testid="player-mode-multi"
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
            <div data-testid="step-session-type" className="space-y-4">
              {([
                { type: "practice" as const, label: "Practice", desc: "Free driving with no timer or AI pressure", icon: "M13 10V3L4 14h7v7l9-11h-7z" },
                { type: "hotlap" as const, label: "Hot Lap", desc: "Timed laps — set the fastest time", icon: "M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" },
                { type: "race" as const, label: "Race", desc: "Full race with grid start and laps", icon: "M3 21v-4m0 0V5a2 2 0 012-2h6.5l1 1H21l-3 6 3 6h-8.5l-1-1H5a2 2 0 00-2 2zm9-13.5V9" },
              ]).map(({ type, label, desc, icon }) => (
                <button
                  key={type}
                  data-testid={`session-type-${type}`}
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
            <div data-testid="step-ai-config" className="space-y-8">
              <div className="flex items-center justify-between px-6 py-4 bg-rp-surface border border-rp-border rounded-xl">
                <div>
                  <p className="text-lg font-semibold text-white">AI Opponents</p>
                  <p className="text-sm text-rp-grey">Race against computer-controlled drivers</p>
                </div>
                <button
                  data-testid="ai-toggle"
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
                          data-testid={`ai-difficulty-${level}`}
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
                      data-testid="ai-count-slider"
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
                data-testid="ai-config-next"
                onClick={() => wizard.goNext()}
                className="w-full py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-xl transition-colors"
              >
                Continue
              </button>
            </div>
          )}

          {/* ─── MULTIPLAYER LOBBY (pod count selector) ──────────── */}
          {step === "multiplayer_lobby" && (
            <div data-testid="step-multiplayer-lobby" className="space-y-8">
              <div className="text-center">
                <h2 className="text-xl font-bold text-white mb-2">How many rigs?</h2>
                <p className="text-rp-grey text-sm">Including your own rig</p>
              </div>

              {/* Pod count grid */}
              <div className="grid grid-cols-4 gap-3">
                {[2, 3, 4, 5, 6, 7, 8].map((n) => (
                  <button
                    key={n}
                    onClick={() => setPodCount(n)}
                    className={`h-20 rounded-xl border-2 text-center transition-all ${
                      podCount === n
                        ? "border-rp-red bg-rp-red/10"
                        : "border-rp-border bg-rp-surface hover:border-rp-red/50"
                    }`}
                  >
                    <span className="text-3xl font-bold text-white">{n}</span>
                    <p className="text-xs text-rp-grey mt-1">rigs</p>
                  </button>
                ))}
              </div>

              <button
                onClick={() => wizard.goNext()}
                className="w-full py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-xl transition-colors"
              >
                Continue with {podCount} Rigs
              </button>
            </div>
          )}

          {/* ─── SELECT EXPERIENCE (preset) ───────────────────────── */}
          {step === "select_experience" && (
            <div data-testid="step-select-experience" className="space-y-4">
              <div className="flex gap-3">
                <button
                  data-testid="experience-mode-preset"
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
                  data-testid="experience-mode-custom"
                  onClick={() => {
                    wizard.setField("experienceMode", "custom");
                    wizard.goToStep("select_track");
                  }}
                  className="flex-1 py-3 rounded-xl text-sm font-medium transition-colors bg-rp-surface border border-rp-border text-rp-grey hover:text-white"
                >
                  Custom (Track + Car)
                </button>
              </div>

              <div data-testid="experience-list" className="space-y-3 max-h-[60vh] overflow-y-auto">
                {experiences.filter((e) => e.game === ws.selectedGame).length === 0 ? (
                  <p className="text-sm text-rp-grey text-center py-8">No experiences configured</p>
                ) : (
                  experiences.filter((e) => e.game === ws.selectedGame).map((exp) => (
                    <button
                      key={exp.id}
                      data-testid={`experience-option-${exp.id}`}
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
            <div data-testid="step-select-track" className="space-y-4">
              <input
                data-testid="track-search"
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
                    data-testid={`track-option-${t.id}`}
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
            <div data-testid="step-select-car" className="space-y-4">
              <input
                data-testid="car-search"
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
                    data-testid={`car-option-${c.id}`}
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
            <div data-testid="step-driving-settings" className="space-y-8">
              <div>
                <h4 className="text-base font-semibold text-white mb-3">Difficulty</h4>
                <div className="grid grid-cols-3 gap-4">
                  {Object.entries(DIFFICULTY_PRESETS).map(([key, preset]) => (
                    <button
                      key={key}
                      data-testid={`difficulty-${key}`}
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
                      data-testid={`transmission-${key}`}
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
                      data-testid={`ffb-${key}`}
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
                data-testid="driving-settings-next"
                onClick={() => wizard.goNext()}
                className="w-full py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-xl transition-colors"
              >
                Review
              </button>
            </div>
          )}

          {/* ─── REVIEW & BOOK ────────────────────────────────────── */}
          {step === "review" && (
            <div data-testid="step-review" className="space-y-6">
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
                {ws.playerMode === "multi" && (
                  <ReviewRow label="Rigs" value={`${podCount} rigs`} />
                )}
              </div>

              <button
                data-testid="book-btn"
                onClick={ws.playerMode === "multi" ? handleBookMultiplayer : handleBook}
                className="w-full py-5 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-xl rounded-xl transition-colors"
              >
                {ws.playerMode === "multi" ? `BOOK ${podCount} RIGS` : "BOOK SESSION"}
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Footer with Back button */}
      <div className="px-6 py-4 border-t border-rp-border shrink-0">
        <button
          data-testid="wizard-back-btn"
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
