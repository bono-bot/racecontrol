"use client";

import { Suspense, useEffect, useState, useMemo, useCallback } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type {
  PricingTier,
  DriverProfile,
  ACCatalog,
  CatalogTrack,
  CatalogCar,
  CustomBookingPayload,
  FriendInfo,
} from "@/lib/api";

// ─── Constants ────────────────────────────────────────────────────────────

const STEP_LABELS_SINGLE = [
  "Duration",
  "Game",
  "Mode",
  "Track",
  "Car",
  "Difficulty",
  "Transmission",
  "Confirm",
];

const STEP_LABELS_MULTI = [
  "Duration",
  "Game",
  "Mode",
  "Friends",
  "Track",
  "Car",
  "Difficulty",
  "Transmission",
  "Confirm",
];

const DIFFICULTY_PRESETS = [
  {
    id: "easy" as const,
    label: "Easy",
    desc: "All assists on — great for beginners",
    aids: ["ABS", "TC", "Stability", "Auto Clutch", "Ideal Line"],
  },
  {
    id: "medium" as const,
    label: "Medium",
    desc: "ABS & TC on, no stability or line",
    aids: ["ABS", "TC", "Auto Clutch"],
  },
  {
    id: "hard" as const,
    label: "Hard",
    desc: "No assists — full control",
    aids: [],
  },
];

// ─── Main ─────────────────────────────────────────────────────────────────

export default function BookPage() {
  return (
    <Suspense fallback={<div className="min-h-screen bg-rp-dark" />}>
      <BookWizard />
    </Suspense>
  );
}

function BookWizard() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const isTrial = searchParams.get("trial") === "true";

  // ── Data state
  const [profile, setProfile] = useState<DriverProfile | null>(null);
  const [tiers, setTiers] = useState<PricingTier[]>([]);
  const [catalog, setCatalog] = useState<ACCatalog | null>(null);
  const [loading, setLoading] = useState(true);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [booking, setBooking] = useState(false);

  // ── Wizard state
  const [step, setStep] = useState(1);
  const [tier, setTier] = useState<PricingTier | null>(null);
  const [game] = useState("assetto_corsa");
  const [mode, setMode] = useState<"single" | "multi">("single");
  const [track, setTrack] = useState<CatalogTrack | null>(null);
  const [car, setCar] = useState<CatalogCar | null>(null);
  const [difficulty, setDifficulty] = useState<"easy" | "medium" | "hard">("easy");
  const [transmission, setTransmission] = useState<"auto" | "manual">("auto");
  const [selectedFriends, setSelectedFriends] = useState<FriendInfo[]>([]);

  // Step labels depend on mode
  const stepLabels = mode === "multi" ? STEP_LABELS_MULTI : STEP_LABELS_SINGLE;
  const totalSteps = stepLabels.length;
  // "Booked" confirmation is totalSteps + 1
  const bookedStep = totalSteps + 1;

  // ── Booking result (PIN + pod)
  const [bookedPin, setBookedPin] = useState<string | null>(null);
  const [bookedPodNumber, setBookedPodNumber] = useState<number>(0);
  const [bookedSeconds, setBookedSeconds] = useState<number>(0);

  // ── Search state for track/car steps
  const [trackSearch, setTrackSearch] = useState("");
  const [carSearch, setCarSearch] = useState("");
  const [showAllTracks, setShowAllTracks] = useState(false);
  const [showAllCars, setShowAllCars] = useState(false);
  const [trackCategory, setTrackCategory] = useState<string | null>(null);
  const [carCategory, setCarCategory] = useState<string | null>(null);

  // ── Logical step mapping: map wizard step to content based on mode
  // Single: 1=Duration, 2=Game, 3=Mode, 4=Track, 5=Car, 6=Difficulty, 7=Transmission, 8=Confirm
  // Multi:  1=Duration, 2=Game, 3=Mode, 4=Friends, 5=Track, 6=Car, 7=Difficulty, 8=Transmission, 9=Confirm
  const stepContent = stepLabels[step - 1];

  // ── Load initial data
  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    async function load() {
      try {
        const [pRes, eRes] = await Promise.all([
          api.profile(),
          api.experiences(),
        ]);
        if (pRes.driver) setProfile(pRes.driver);
        if (eRes.pricing_tiers) setTiers(eRes.pricing_tiers);

        // Active reservation → redirect
        if (pRes.driver?.active_reservation) {
          router.push("/book/active");
          return;
        }

        // Trial auto-select
        if (isTrial && eRes.pricing_tiers) {
          const trialTier = eRes.pricing_tiers.find((t) => t.is_trial);
          if (trialTier) {
            setTier(trialTier);
            setStep(2);
          }
        }
      } catch {
        setError("Failed to load data");
      } finally {
        setLoading(false);
      }
    }
    load();
  }, [isTrial, router]);

  // ── Load catalog when reaching step 4
  const loadCatalog = useCallback(async () => {
    if (catalog || catalogLoading) return;
    setCatalogLoading(true);
    try {
      const res = await api.acCatalog();
      if (res.tracks && res.cars) {
        setCatalog(res);
      } else if (res.error) {
        setError(res.error);
      }
    } catch {
      setError("Failed to load catalog");
    } finally {
      setCatalogLoading(false);
    }
  }, [catalog, catalogLoading]);

  useEffect(() => {
    if (stepContent === "Track") loadCatalog();
  }, [step, stepContent, loadCatalog]);

  // ── Navigation
  function goNext() {
    if (step < totalSteps) setStep(step + 1);
  }

  function goBack() {
    if (step > 1) {
      setStep(step - 1);
    } else {
      router.push("/dashboard");
    }
  }

  // ── Booking
  async function handleBook() {
    if (!tier || !track || !car) return;
    setBooking(true);
    setError(null);

    const custom: CustomBookingPayload = {
      game,
      game_mode: mode,
      track: track.id,
      car: car.id,
      difficulty,
      transmission,
    };

    try {
      if (mode === "multi" && selectedFriends.length > 0) {
        // Multiplayer booking
        const friendIds = selectedFriends.map((f) => f.driver_id);
        const res = await api.bookMultiplayer(tier.id, friendIds, undefined, custom);
        if (res.group_session) {
          router.push("/book/group");
        } else {
          setError(res.error || "Multiplayer booking failed");
        }
      } else {
        // Single player booking
        const res = await api.bookCustom(tier.id, custom);
        if (res.status === "booked" && res.pin) {
          setBookedPin(res.pin);
          setBookedPodNumber(res.pod_number || 0);
          setBookedSeconds(res.allocated_seconds || 0);
          setStep(bookedStep);
        } else {
          setError(res.error || "Booking failed");
        }
      }
    } catch {
      setError("Network error");
    } finally {
      setBooking(false);
    }
  }

  // ── Loading
  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  // Booked confirmation — full-screen, no wizard header
  if (step === bookedStep && bookedPin) {
    return (
      <div className="min-h-screen pb-24 px-4">
        <BookedPinScreen
          pin={bookedPin}
          podNumber={bookedPodNumber}
          allocatedSeconds={bookedSeconds}
          onContinue={() => router.push("/book/active")}
        />
      </div>
    );
  }

  return (
    <div className="min-h-screen pb-24">
      {/* Header */}
      <div className="px-4 pt-6 pb-4">
        <div className="flex items-center gap-3 mb-4">
          <button
            onClick={goBack}
            className="w-10 h-10 flex items-center justify-center rounded-xl bg-rp-card border border-rp-border"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-white">
              <path d="M15 18l-6-6 6-6" />
            </svg>
          </button>
          <div className="flex-1">
            <h1 className="text-lg font-bold text-white">{stepLabels[step - 1]}</h1>
            <p className="text-xs text-rp-grey">Step {step} of {totalSteps}</p>
          </div>
          <div className="text-right">
            <p className="text-xs text-rp-grey">Credits</p>
            <p className="text-sm font-bold text-white">
              {((profile?.wallet_balance_paise || 0) / 100).toFixed(0)}
            </p>
          </div>
        </div>

        {/* Step indicator */}
        <div className="flex gap-1">
          {stepLabels.map((_, i) => (
            <div
              key={i}
              className={`h-1 flex-1 rounded-full transition-colors ${
                i < step ? "bg-rp-red" : "bg-rp-border"
              }`}
            />
          ))}
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="mx-4 mb-4 bg-red-900/30 border border-red-500/30 rounded-xl p-3 text-red-400 text-sm">
          {error}
          <button onClick={() => setError(null)} className="ml-2 underline">Dismiss</button>
        </div>
      )}

      {/* Step content */}
      <div className="px-4">
        {stepContent === "Duration" && (
          <DurationStep
            tiers={tiers}
            selected={tier}
            isTrial={isTrial}
            onSelect={(t) => {
              setTier(t);
              goNext();
            }}
          />
        )}
        {stepContent === "Game" && (
          <GameStep
            onSelect={() => goNext()}
          />
        )}
        {stepContent === "Mode" && (
          <ModeStep
            selected={mode}
            onSelect={(m) => {
              setMode(m);
              goNext();
            }}
          />
        )}
        {stepContent === "Friends" && (
          <FriendsPickerStep
            selected={selectedFriends}
            onSelect={setSelectedFriends}
            onContinue={goNext}
          />
        )}
        {stepContent === "Track" && (
          <TrackStep
            catalog={catalog}
            loading={catalogLoading}
            selected={track}
            search={trackSearch}
            onSearchChange={setTrackSearch}
            showAll={showAllTracks}
            onToggleAll={() => setShowAllTracks(!showAllTracks)}
            category={trackCategory}
            onCategoryChange={setTrackCategory}
            onSelect={(t) => {
              setTrack(t);
              goNext();
            }}
          />
        )}
        {stepContent === "Car" && (
          <CarStep
            catalog={catalog}
            loading={catalogLoading}
            selected={car}
            search={carSearch}
            onSearchChange={setCarSearch}
            showAll={showAllCars}
            onToggleAll={() => setShowAllCars(!showAllCars)}
            category={carCategory}
            onCategoryChange={setCarCategory}
            onSelect={(c) => {
              setCar(c);
              goNext();
            }}
          />
        )}
        {stepContent === "Difficulty" && (
          <DifficultyStep
            selected={difficulty}
            onSelect={(d) => {
              setDifficulty(d);
              goNext();
            }}
          />
        )}
        {stepContent === "Transmission" && (
          <TransmissionStep
            selected={transmission}
            onSelect={(t) => {
              setTransmission(t);
              goNext();
            }}
          />
        )}
        {stepContent === "Confirm" && (
          <ConfirmStep
            tier={tier}
            track={track}
            car={car}
            difficulty={difficulty}
            transmission={transmission}
            mode={mode}
            selectedFriends={selectedFriends}
            balance={profile?.wallet_balance_paise || 0}
            booking={booking}
            onBook={handleBook}
          />
        )}
      </div>
    </div>
  );
}

// ─── Step Components ──────────────────────────────────────────────────────

function DurationStep({
  tiers,
  selected,
  isTrial,
  onSelect,
}: {
  tiers: PricingTier[];
  selected: PricingTier | null;
  isTrial: boolean;
  onSelect: (t: PricingTier) => void;
}) {
  const displayTiers = isTrial ? tiers.filter((t) => t.is_trial) : tiers.filter((t) => !t.is_trial);

  return (
    <div className="space-y-3">
      <p className="text-sm text-rp-grey mb-2">How long do you want to race?</p>
      {displayTiers.map((t) => (
        <button
          key={t.id}
          onClick={() => onSelect(t)}
          className={`w-full text-left bg-rp-card border rounded-xl p-4 transition-colors ${
            selected?.id === t.id ? "border-rp-red" : "border-rp-border"
          }`}
        >
          <div className="flex items-center justify-between">
            <div>
              <p className="text-white font-semibold text-lg">{t.name}</p>
              <p className="text-rp-grey text-sm">{t.duration_minutes} minutes</p>
            </div>
            <div className="text-right">
              <p className="text-white font-bold text-xl">
                {t.is_trial ? "Free" : `${(t.price_paise / 100).toFixed(0)}`}
              </p>
              {!t.is_trial && <p className="text-rp-grey text-xs">credits</p>}
            </div>
          </div>
        </button>
      ))}
    </div>
  );
}

function GameStep({ onSelect }: { onSelect: () => void }) {
  return (
    <div className="space-y-3">
      <p className="text-sm text-rp-grey mb-2">Select your sim</p>
      <button
        onClick={onSelect}
        className="w-full text-left bg-rp-card border border-rp-red rounded-xl p-5 transition-colors"
      >
        <div className="flex items-center gap-4">
          <div className="w-12 h-12 rounded-lg bg-neutral-800 flex items-center justify-center text-2xl font-bold text-rp-red">
            AC
          </div>
          <div>
            <p className="text-white font-semibold text-lg">Assetto Corsa</p>
            <p className="text-rp-grey text-sm">51 tracks, 280+ cars</p>
          </div>
        </div>
      </button>
      {/* Future games — disabled */}
      {["F1 25", "iRacing", "Le Mans Ultimate"].map((g) => (
        <div
          key={g}
          className="w-full text-left bg-rp-card border border-rp-border rounded-xl p-5 opacity-40"
        >
          <div className="flex items-center gap-4">
            <div className="w-12 h-12 rounded-lg bg-neutral-800 flex items-center justify-center text-sm font-bold text-rp-grey">
              {g.slice(0, 2).toUpperCase()}
            </div>
            <div>
              <p className="text-rp-grey font-semibold">{g}</p>
              <p className="text-rp-grey text-sm">Coming soon</p>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function ModeStep({
  selected,
  onSelect,
}: {
  selected: "single" | "multi";
  onSelect: (m: "single" | "multi") => void;
}) {
  return (
    <div className="space-y-3">
      <p className="text-sm text-rp-grey mb-2">Race solo or with friends?</p>
      <button
        onClick={() => onSelect("single")}
        className={`w-full text-left bg-rp-card border rounded-xl p-5 transition-colors ${
          selected === "single" ? "border-rp-red" : "border-rp-border"
        }`}
      >
        <p className="text-white font-semibold text-lg">Single Player</p>
        <p className="text-rp-grey text-sm mt-1">Race at your own pace</p>
      </button>
      <button
        onClick={() => onSelect("multi")}
        className={`w-full text-left bg-rp-card border rounded-xl p-5 transition-colors ${
          selected === "multi" ? "border-rp-red" : "border-rp-border"
        }`}
      >
        <p className="text-white font-semibold text-lg">Multiplayer</p>
        <p className="text-rp-grey text-sm mt-1">Race against friends on LAN</p>
      </button>
    </div>
  );
}

function FriendsPickerStep({
  selected,
  onSelect,
  onContinue,
}: {
  selected: FriendInfo[];
  onSelect: (friends: FriendInfo[]) => void;
  onContinue: () => void;
}) {
  const [friends, setFriends] = useState<FriendInfo[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.friends().then((res) => {
      if (res.friends) setFriends(res.friends);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  function toggle(f: FriendInfo) {
    const exists = selected.find((s) => s.driver_id === f.driver_id);
    if (exists) {
      onSelect(selected.filter((s) => s.driver_id !== f.driver_id));
    } else if (selected.length < 7) {
      onSelect([...selected, f]);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  const onlineFriends = friends.filter((f) => f.is_online);
  const offlineFriends = friends.filter((f) => !f.is_online);

  if (friends.length === 0) {
    return (
      <div className="text-center py-12">
        <p className="text-rp-grey mb-4">No friends added yet</p>
        <a href="/friends" className="text-rp-red font-medium text-sm">
          Add friends first
        </a>
      </div>
    );
  }

  function formatTime(ms: number) {
    const hrs = Math.floor(ms / 3600000);
    const mins = Math.floor((ms % 3600000) / 60000);
    return hrs > 0 ? `${hrs}h ${mins}m` : `${mins}m`;
  }

  return (
    <div>
      <p className="text-sm text-rp-grey mb-3">
        Select friends to race with (max 7)
      </p>

      {onlineFriends.length > 0 && (
        <>
          <p className="text-xs text-emerald-400 uppercase tracking-wide mb-2">Online</p>
          <div className="space-y-2 mb-4">
            {onlineFriends.map((f) => {
              const isSelected = selected.some((s) => s.driver_id === f.driver_id);
              return (
                <button
                  key={f.driver_id}
                  onClick={() => toggle(f)}
                  className={`w-full text-left bg-rp-card border rounded-xl p-3.5 transition-colors ${
                    isSelected ? "border-rp-red" : "border-rp-border"
                  }`}
                >
                  <div className="flex items-center gap-3">
                    <div className="relative">
                      <div className="w-10 h-10 rounded-full bg-neutral-700 flex items-center justify-center text-white font-bold text-sm">
                        {f.name.charAt(0).toUpperCase()}
                      </div>
                      <span className="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-emerald-400 rounded-full border-2 border-rp-card" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="text-white font-medium text-sm truncate">{f.name}</p>
                      <p className="text-rp-grey text-xs">
                        {f.total_laps} laps &middot; {formatTime(f.total_time_ms)} &middot; {f.session_count} sessions
                      </p>
                    </div>
                    {isSelected && (
                      <div className="w-6 h-6 rounded-full bg-rp-red flex items-center justify-center">
                        <svg className="w-4 h-4 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                        </svg>
                      </div>
                    )}
                  </div>
                </button>
              );
            })}
          </div>
        </>
      )}

      {offlineFriends.length > 0 && (
        <>
          <p className="text-xs text-rp-grey uppercase tracking-wide mb-2">Offline</p>
          <div className="space-y-2 mb-4 opacity-50">
            {offlineFriends.map((f) => (
              <div
                key={f.driver_id}
                className="w-full text-left bg-rp-card border border-rp-border rounded-xl p-3.5"
              >
                <div className="flex items-center gap-3">
                  <div className="w-10 h-10 rounded-full bg-neutral-700 flex items-center justify-center text-white font-bold text-sm">
                    {f.name.charAt(0).toUpperCase()}
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-rp-grey font-medium text-sm truncate">{f.name}</p>
                    <p className="text-rp-grey text-xs">
                      {f.total_laps} laps &middot; {formatTime(f.total_time_ms)}
                    </p>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {onlineFriends.length === 0 && (
        <div className="text-center py-8 mb-4">
          <p className="text-rp-grey text-sm">No friends online right now</p>
          <a href="/friends" className="text-rp-red font-medium text-xs mt-2 inline-block">
            Add more friends
          </a>
        </div>
      )}

      <button
        onClick={onContinue}
        disabled={selected.length === 0}
        className="w-full bg-rp-red text-white font-semibold py-4 rounded-xl text-lg disabled:opacity-50 transition-opacity"
      >
        Continue with {selected.length} friend{selected.length !== 1 ? "s" : ""}
      </button>
    </div>
  );
}

function TrackStep({
  catalog,
  loading,
  selected,
  search,
  onSearchChange,
  showAll,
  onToggleAll,
  category,
  onCategoryChange,
  onSelect,
}: {
  catalog: ACCatalog | null;
  loading: boolean;
  selected: CatalogTrack | null;
  search: string;
  onSearchChange: (s: string) => void;
  showAll: boolean;
  onToggleAll: () => void;
  category: string | null;
  onCategoryChange: (c: string | null) => void;
  onSelect: (t: CatalogTrack) => void;
}) {
  const tracks = useMemo(() => {
    if (!catalog) return [];
    const source = showAll ? catalog.tracks.all : catalog.tracks.featured;
    let filtered = source;

    if (search) {
      const q = search.toLowerCase();
      filtered = filtered.filter(
        (t) =>
          t.name.toLowerCase().includes(q) ||
          t.category.toLowerCase().includes(q) ||
          (t.country && t.country.toLowerCase().includes(q))
      );
    }

    if (category) {
      filtered = filtered.filter((t) => t.category === category);
    }

    return filtered;
  }, [catalog, showAll, search, category]);

  const categories = catalog?.categories.tracks || [];

  // Group by category
  const grouped = useMemo(() => {
    const groups: Record<string, CatalogTrack[]> = {};
    for (const t of tracks) {
      if (!groups[t.category]) groups[t.category] = [];
      groups[t.category].push(t);
    }
    return groups;
  }, [tracks]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (!catalog) {
    return <p className="text-rp-grey text-center py-8">Catalog unavailable</p>;
  }

  return (
    <div>
      <p className="text-sm text-rp-grey mb-3">Choose your circuit</p>

      {/* Search */}
      <input
        type="text"
        value={search}
        onChange={(e) => onSearchChange(e.target.value)}
        placeholder="Search tracks..."
        className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3 text-white text-sm placeholder:text-rp-grey focus:outline-none focus:border-rp-red mb-3"
      />

      {/* Category tabs */}
      {showAll && (
        <div className="flex gap-2 mb-4 overflow-x-auto no-scrollbar">
          <button
            onClick={() => onCategoryChange(null)}
            className={`px-3 py-1.5 rounded-lg text-xs font-medium whitespace-nowrap ${
              !category ? "bg-rp-red text-white" : "bg-rp-card text-rp-grey border border-rp-border"
            }`}
          >
            All
          </button>
          {categories.map((c) => (
            <button
              key={c}
              onClick={() => onCategoryChange(c)}
              className={`px-3 py-1.5 rounded-lg text-xs font-medium whitespace-nowrap ${
                category === c ? "bg-rp-red text-white" : "bg-rp-card text-rp-grey border border-rp-border"
              }`}
            >
              {c}
            </button>
          ))}
        </div>
      )}

      {/* Track list */}
      <div className="space-y-4 max-h-[60vh] overflow-y-auto">
        {Object.entries(grouped).map(([cat, items]) => (
          <div key={cat}>
            <p className="text-rp-grey text-xs uppercase tracking-wide mb-2">{cat}</p>
            <div className="space-y-2">
              {items.map((t) => (
                <button
                  key={t.id}
                  onClick={() => onSelect(t)}
                  className={`w-full text-left bg-rp-card border rounded-xl p-3.5 transition-colors ${
                    selected?.id === t.id ? "border-rp-red" : "border-rp-border"
                  }`}
                >
                  <p className="text-white font-medium text-sm">{t.name}</p>
                  {t.country && (
                    <p className="text-rp-grey text-xs mt-0.5">{t.country}</p>
                  )}
                </button>
              ))}
            </div>
          </div>
        ))}
      </div>

      {/* Show all toggle */}
      {!search && (
        <button
          onClick={onToggleAll}
          className="w-full mt-4 py-3 text-center text-sm text-rp-red font-medium"
        >
          {showAll
            ? "Show Featured"
            : `Show All (${catalog.tracks.all.length} tracks)`}
        </button>
      )}
    </div>
  );
}

function CarStep({
  catalog,
  loading,
  selected,
  search,
  onSearchChange,
  showAll,
  onToggleAll,
  category,
  onCategoryChange,
  onSelect,
}: {
  catalog: ACCatalog | null;
  loading: boolean;
  selected: CatalogCar | null;
  search: string;
  onSearchChange: (s: string) => void;
  showAll: boolean;
  onToggleAll: () => void;
  category: string | null;
  onCategoryChange: (c: string | null) => void;
  onSelect: (c: CatalogCar) => void;
}) {
  const cars = useMemo(() => {
    if (!catalog) return [];
    const source = showAll ? catalog.cars.all : catalog.cars.featured;
    let filtered = source;

    if (search) {
      const q = search.toLowerCase();
      filtered = filtered.filter(
        (c) =>
          c.name.toLowerCase().includes(q) ||
          c.category.toLowerCase().includes(q)
      );
    }

    if (category) {
      filtered = filtered.filter((c) => c.category === category);
    }

    return filtered;
  }, [catalog, showAll, search, category]);

  const categories = catalog?.categories.cars || [];

  const grouped = useMemo(() => {
    const groups: Record<string, CatalogCar[]> = {};
    for (const c of cars) {
      if (!groups[c.category]) groups[c.category] = [];
      groups[c.category].push(c);
    }
    return groups;
  }, [cars]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (!catalog) {
    return <p className="text-rp-grey text-center py-8">Catalog unavailable</p>;
  }

  return (
    <div>
      <p className="text-sm text-rp-grey mb-3">Choose your machine</p>

      {/* Search */}
      <input
        type="text"
        value={search}
        onChange={(e) => onSearchChange(e.target.value)}
        placeholder="Search cars..."
        className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3 text-white text-sm placeholder:text-rp-grey focus:outline-none focus:border-rp-red mb-3"
      />

      {/* Category tabs */}
      {showAll && (
        <div className="flex gap-2 mb-4 overflow-x-auto no-scrollbar">
          <button
            onClick={() => onCategoryChange(null)}
            className={`px-3 py-1.5 rounded-lg text-xs font-medium whitespace-nowrap ${
              !category ? "bg-rp-red text-white" : "bg-rp-card text-rp-grey border border-rp-border"
            }`}
          >
            All
          </button>
          {categories.map((c) => (
            <button
              key={c}
              onClick={() => onCategoryChange(c)}
              className={`px-3 py-1.5 rounded-lg text-xs font-medium whitespace-nowrap ${
                category === c ? "bg-rp-red text-white" : "bg-rp-card text-rp-grey border border-rp-border"
              }`}
            >
              {c}
            </button>
          ))}
        </div>
      )}

      {/* Car list */}
      <div className="space-y-4 max-h-[60vh] overflow-y-auto">
        {Object.entries(grouped).map(([cat, items]) => (
          <div key={cat}>
            <p className="text-rp-grey text-xs uppercase tracking-wide mb-2">{cat}</p>
            <div className="space-y-2">
              {items.map((c) => (
                <button
                  key={c.id}
                  onClick={() => onSelect(c)}
                  className={`w-full text-left bg-rp-card border rounded-xl p-3.5 transition-colors ${
                    selected?.id === c.id ? "border-rp-red" : "border-rp-border"
                  }`}
                >
                  <p className="text-white font-medium text-sm">{c.name}</p>
                </button>
              ))}
            </div>
          </div>
        ))}
      </div>

      {/* Show all toggle */}
      {!search && (
        <button
          onClick={onToggleAll}
          className="w-full mt-4 py-3 text-center text-sm text-rp-red font-medium"
        >
          {showAll
            ? "Show Featured"
            : `Show All (${catalog.cars.all.length} cars)`}
        </button>
      )}
    </div>
  );
}

function DifficultyStep({
  selected,
  onSelect,
}: {
  selected: "easy" | "medium" | "hard";
  onSelect: (d: "easy" | "medium" | "hard") => void;
}) {
  return (
    <div className="space-y-3">
      <p className="text-sm text-rp-grey mb-2">Choose your challenge level</p>
      {DIFFICULTY_PRESETS.map((d) => (
        <button
          key={d.id}
          onClick={() => onSelect(d.id)}
          className={`w-full text-left bg-rp-card border rounded-xl p-4 transition-colors ${
            selected === d.id ? "border-rp-red" : "border-rp-border"
          }`}
        >
          <div className="flex items-center justify-between mb-1">
            <p className="text-white font-semibold text-lg">{d.label}</p>
            <span className="text-lg">
              {d.id === "easy" ? "\u{1F60A}" : d.id === "medium" ? "\u{1F60E}" : "\u{1F525}"}
            </span>
          </div>
          <p className="text-rp-grey text-sm mb-2">{d.desc}</p>
          <div className="flex flex-wrap gap-1.5">
            {d.aids.length > 0 ? (
              d.aids.map((a) => (
                <span
                  key={a}
                  className="text-xs bg-emerald-900/40 text-emerald-400 px-2 py-0.5 rounded"
                >
                  {a}
                </span>
              ))
            ) : (
              <span className="text-xs bg-red-900/40 text-red-400 px-2 py-0.5 rounded">
                No Assists
              </span>
            )}
          </div>
        </button>
      ))}
    </div>
  );
}

function TransmissionStep({
  selected,
  onSelect,
}: {
  selected: "auto" | "manual";
  onSelect: (t: "auto" | "manual") => void;
}) {
  return (
    <div className="space-y-3">
      <p className="text-sm text-rp-grey mb-2">Gearbox preference</p>
      <button
        onClick={() => onSelect("auto")}
        className={`w-full text-left bg-rp-card border rounded-xl p-5 transition-colors ${
          selected === "auto" ? "border-rp-red" : "border-rp-border"
        }`}
      >
        <p className="text-white font-semibold text-lg">Automatic</p>
        <p className="text-rp-grey text-sm mt-1">Car shifts for you — focus on driving</p>
      </button>
      <button
        onClick={() => onSelect("manual")}
        className={`w-full text-left bg-rp-card border rounded-xl p-5 transition-colors ${
          selected === "manual" ? "border-rp-red" : "border-rp-border"
        }`}
      >
        <p className="text-white font-semibold text-lg">Manual</p>
        <p className="text-rp-grey text-sm mt-1">Use paddle shifters — full control</p>
      </button>
    </div>
  );
}

function ConfirmStep({
  tier,
  track,
  car,
  difficulty,
  transmission,
  mode,
  selectedFriends,
  balance,
  booking,
  onBook,
}: {
  tier: PricingTier | null;
  track: CatalogTrack | null;
  car: CatalogCar | null;
  difficulty: string;
  transmission: string;
  mode: string;
  selectedFriends: FriendInfo[];
  balance: number;
  booking: boolean;
  onBook: () => void;
}) {
  const price = tier?.price_paise || 0;
  const canAfford = tier?.is_trial || balance >= price;

  const rows = [
    { label: "Duration", value: tier?.name || "—" },
    { label: "Game", value: "Assetto Corsa" },
    { label: "Mode", value: mode === "single" ? "Single Player" : "Multiplayer" },
    { label: "Track", value: track?.name || "—" },
    { label: "Car", value: car?.name || "—" },
    { label: "Difficulty", value: difficulty.charAt(0).toUpperCase() + difficulty.slice(1) },
    { label: "Transmission", value: transmission === "auto" ? "Automatic" : "Manual" },
  ];

  return (
    <div>
      <p className="text-sm text-rp-grey mb-4">Review your race setup</p>

      <div className="bg-rp-card border border-rp-border rounded-xl divide-y divide-rp-border">
        {rows.map((r) => (
          <div key={r.label} className="flex items-center justify-between px-4 py-3">
            <span className="text-rp-grey text-sm">{r.label}</span>
            <span className="text-white text-sm font-medium">{r.value}</span>
          </div>
        ))}
      </div>

      {/* Friends for multiplayer */}
      {mode === "multi" && selectedFriends.length > 0 && (
        <div className="mt-4 bg-rp-card border border-rp-border rounded-xl p-4">
          <p className="text-rp-grey text-xs uppercase tracking-wide mb-2">Racing with</p>
          <div className="flex flex-wrap gap-2">
            {selectedFriends.map((f) => (
              <span key={f.driver_id} className="bg-neutral-700 text-white text-xs px-2.5 py-1 rounded-lg">
                {f.name}
              </span>
            ))}
          </div>
          <p className="text-rp-grey text-xs mt-3">
            Each friend pays their own share from their wallet
          </p>
        </div>
      )}

      {/* Price summary */}
      <div className="mt-4 bg-rp-card border border-rp-border rounded-xl p-4">
        <div className="flex items-center justify-between mb-1">
          <span className="text-rp-grey text-sm">Cost</span>
          <span className="text-white font-bold text-lg">
            {tier?.is_trial ? "Free Trial" : `${(price / 100).toFixed(0)} credits`}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-rp-grey text-sm">Your balance</span>
          <span className={`text-sm font-medium ${canAfford ? "text-emerald-400" : "text-red-400"}`}>
            {(balance / 100).toFixed(0)} credits
          </span>
        </div>
      </div>

      {!canAfford && (
        <div className="mt-3 bg-red-900/30 border border-red-500/30 rounded-xl p-3 text-red-400 text-sm">
          Insufficient credits. You need {((price - balance) / 100).toFixed(0)} more credits.
        </div>
      )}

      {/* Book button */}
      <button
        onClick={onBook}
        disabled={booking || !canAfford}
        className="w-full mt-6 bg-rp-red text-white font-semibold py-4 rounded-xl text-lg disabled:opacity-50 transition-opacity"
      >
        {booking
          ? "Booking..."
          : tier?.is_trial
          ? "Start Free Trial"
          : `Debit ${(price / 100).toFixed(0)} credits & Race`}
      </button>
    </div>
  );
}

function BookedPinScreen({
  pin,
  podNumber,
  allocatedSeconds,
  onContinue,
}: {
  pin: string;
  podNumber: number;
  allocatedSeconds: number;
  onContinue: () => void;
}) {
  const minutes = Math.floor(allocatedSeconds / 60);

  return (
    <div className="flex flex-col items-center justify-center min-h-[80vh] text-center">
      {/* Checkmark */}
      <div className="w-16 h-16 rounded-full bg-emerald-900/30 flex items-center justify-center mb-6">
        <svg className="w-8 h-8 text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
        </svg>
      </div>

      <h1 className="text-2xl font-bold text-white mb-2">Booked!</h1>
      <p className="text-rp-grey mb-8">Enter this PIN at the kiosk terminal</p>

      {/* Large PIN display */}
      <div className="flex gap-3 justify-center mb-8">
        {pin.split("").map((digit, i) => (
          <div
            key={i}
            className="w-16 h-20 bg-rp-card border-2 border-rp-red rounded-xl flex items-center justify-center"
          >
            <span className="text-4xl font-bold text-white">{digit}</span>
          </div>
        ))}
      </div>

      {/* Info */}
      <div className="bg-rp-card border border-rp-border rounded-xl p-4 w-full max-w-xs space-y-2 mb-8">
        <div className="flex justify-between">
          <span className="text-rp-grey text-sm">Rig</span>
          <span className="text-white font-bold text-sm">#{podNumber}</span>
        </div>
        <div className="flex justify-between">
          <span className="text-rp-grey text-sm">Duration</span>
          <span className="text-white font-bold text-sm">{minutes} min</span>
        </div>
      </div>

      <button
        onClick={onContinue}
        className="w-full max-w-xs bg-rp-red text-white font-semibold py-4 rounded-xl text-lg"
      >
        View Session
      </button>
    </div>
  );
}
