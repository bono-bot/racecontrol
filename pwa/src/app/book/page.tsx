"use client";

import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { api } from "@/lib/api";
import type { Experience, PricingTier, DriverProfile } from "@/lib/api";

export default function BookPage() {
  return (
    <Suspense fallback={<div className="min-h-screen bg-rp-black" />}>
      <BookPageInner />
    </Suspense>
  );
}

function BookPageInner() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const isTrial = searchParams.get("trial") === "true";

  const [profile, setProfile] = useState<DriverProfile | null>(null);
  const [experiences, setExperiences] = useState<Experience[]>([]);
  const [tiers, setTiers] = useState<PricingTier[]>([]);
  const [selectedExp, setSelectedExp] = useState<Experience | null>(null);
  const [selectedTier, setSelectedTier] = useState<PricingTier | null>(null);
  const [activeGame, setActiveGame] = useState<string>("all");
  const [booking, setBooking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function load() {
      try {
        const [pRes, eRes] = await Promise.all([
          api.profile(),
          api.experiences(),
        ]);
        if (pRes.driver) setProfile(pRes.driver);
        if (eRes.experiences) setExperiences(eRes.experiences);
        if (eRes.pricing_tiers) setTiers(eRes.pricing_tiers);

        // Check for active reservation
        if (pRes.driver?.active_reservation) {
          router.push("/book/active");
          return;
        }

        // Auto-select trial
        if (isTrial && eRes.pricing_tiers) {
          const trialTier = eRes.pricing_tiers.find((t) => t.is_trial);
          const trialExp = eRes.experiences?.find((e) => e.id === "exp_trial");
          if (trialTier) setSelectedTier(trialTier);
          if (trialExp) setSelectedExp(trialExp);
        }
      } catch {
        setError("Failed to load experiences");
      } finally {
        setLoading(false);
      }
    }
    load();
  }, [isTrial, router]);

  const games = Array.from(new Set(experiences.map((e) => e.game)));
  const filteredExps =
    activeGame === "all"
      ? experiences
      : experiences.filter((e) => e.game === activeGame);
  const nonTrialTiers = tiers.filter((t) => !t.is_trial);

  async function handleBook() {
    if (!selectedExp || !selectedTier) return;
    setBooking(true);
    setError(null);

    try {
      const res = await api.bookSession(selectedExp.id, selectedTier.id);
      if (res.status === "booked") {
        router.push("/book/active");
      } else {
        setError(res.error || "Booking failed");
      }
    } catch {
      setError("Network error");
    } finally {
      setBooking(false);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="px-4 pt-12 pb-24 max-w-lg mx-auto">
      {/* Wallet balance */}
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">Race Now</h1>
        <div className="text-right">
          <p className="text-xs text-rp-grey">Balance</p>
          <p className="text-lg font-bold text-white">
            {"\u20B9"}
            {((profile?.wallet_balance_paise || 0) / 100).toFixed(0)}
          </p>
        </div>
      </div>

      {error && (
        <div className="bg-red-900/30 border border-red-500/30 rounded-xl p-3 mb-4 text-red-400 text-sm">
          {error}
        </div>
      )}

      {/* Game tabs */}
      <div className="flex gap-2 mb-4 overflow-x-auto no-scrollbar">
        <button
          onClick={() => setActiveGame("all")}
          className={`px-3 py-1.5 rounded-lg text-sm font-medium whitespace-nowrap ${
            activeGame === "all"
              ? "bg-rp-red text-white"
              : "bg-rp-card text-rp-grey border border-rp-border"
          }`}
        >
          All
        </button>
        {games.map((g) => (
          <button
            key={g}
            onClick={() => setActiveGame(g)}
            className={`px-3 py-1.5 rounded-lg text-sm font-medium whitespace-nowrap ${
              activeGame === g
                ? "bg-rp-red text-white"
                : "bg-rp-card text-rp-grey border border-rp-border"
            }`}
          >
            {formatGameName(g)}
          </button>
        ))}
      </div>

      {/* Experiences grid */}
      <div className="space-y-3 mb-6">
        {filteredExps
          .filter((e) => !isTrial || e.id === "exp_trial")
          .map((exp) => (
            <button
              key={exp.id}
              onClick={() => setSelectedExp(exp)}
              className={`w-full text-left bg-rp-card border rounded-xl p-4 transition-colors ${
                selectedExp?.id === exp.id
                  ? "border-rp-red"
                  : "border-rp-border"
              }`}
            >
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-white font-semibold">{exp.name}</p>
                  <p className="text-rp-grey text-xs mt-0.5">
                    {exp.track} &middot; {exp.car}
                  </p>
                </div>
                <span className="text-xs text-rp-grey bg-neutral-800 px-2 py-1 rounded">
                  {formatGameName(exp.game)}
                </span>
              </div>
            </button>
          ))}
      </div>

      {/* Pricing tiers */}
      {selectedExp && !isTrial && (
        <div className="mb-6">
          <h2 className="text-sm font-medium text-rp-grey mb-3">
            Select Duration
          </h2>
          <div className="grid grid-cols-2 gap-3">
            {nonTrialTiers.map((tier) => (
              <button
                key={tier.id}
                onClick={() => setSelectedTier(tier)}
                className={`bg-rp-card border rounded-xl p-4 text-center transition-colors ${
                  selectedTier?.id === tier.id
                    ? "border-rp-red"
                    : "border-rp-border"
                }`}
              >
                <p className="text-white font-bold text-lg">
                  {"\u20B9"}{(tier.price_paise / 100).toFixed(0)}
                </p>
                <p className="text-rp-grey text-xs">{tier.name}</p>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Confirm button */}
      {selectedExp && selectedTier && (
        <div className="fixed bottom-20 left-0 right-0 px-4 max-w-lg mx-auto">
          <button
            onClick={handleBook}
            disabled={booking}
            className="w-full bg-rp-red text-white font-semibold py-4 rounded-xl text-lg disabled:opacity-50"
          >
            {booking
              ? "Booking..."
              : isTrial
              ? "Start Free Trial"
              : `Debit \u20B9${(selectedTier.price_paise / 100).toFixed(0)} for ${selectedTier.name}`}
          </button>
        </div>
      )}
    </div>
  );
}

function formatGameName(game: string): string {
  const names: Record<string, string> = {
    assetto_corsa: "Assetto Corsa",
    iracing: "iRacing",
    f1_25: "F1 25",
    le_mans_ultimate: "LMU",
    forza: "Forza",
  };
  return names[game] || game;
}
