"use client";

import { useEffect, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type {
  FriendInfo,
  PricingTier,
  CatalogTrack,
  CatalogCar,
  ACCatalog,
} from "@/lib/api";

// ─── Steps ───────────────────────────────────────────────────────────────────

const STEPS = ["Friends", "Configure", "Confirm"];

// ─── Main ────────────────────────────────────────────────────────────────────

export default function MultiplayerPage() {
  const router = useRouter();
  const [step, setStep] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Data
  const [friends, setFriends] = useState<FriendInfo[]>([]);
  const [tiers, setTiers] = useState<PricingTier[]>([]);
  const [catalog, setCatalog] = useState<ACCatalog | null>(null);

  // Selections
  const [selectedFriends, setSelectedFriends] = useState<string[]>([]);
  const [selectedTier, setSelectedTier] = useState<string | null>(null);
  const [selectedTrack, setSelectedTrack] = useState<string | null>(null);
  const [selectedCar, setSelectedCar] = useState<string | null>(null);

  // Submission
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    Promise.all([api.friends(), api.experiences(), api.acCatalog()])
      .then(([fRes, eRes, cRes]) => {
        if (fRes.friends) setFriends(fRes.friends);
        if (eRes.pricing_tiers) {
          setTiers(eRes.pricing_tiers.filter((t) => !t.is_trial));
        }
        if (cRes.tracks) setCatalog(cRes);
        setLoading(false);
      })
      .catch(() => {
        setError("Failed to load data");
        setLoading(false);
      });
  }, [router]);

  const onlineFriends = friends.filter((f) => f.is_online);
  const selectedTierObj = tiers.find((t) => t.id === selectedTier) || null;

  const toggleFriend = useCallback((driverId: string) => {
    setSelectedFriends((prev) => {
      if (prev.includes(driverId)) {
        return prev.filter((id) => id !== driverId);
      }
      if (prev.length >= 7) return prev; // Max 7 friends (8 pods minus host)
      return [...prev, driverId];
    });
  }, []);

  const handleSubmit = async () => {
    if (!selectedTier || selectedFriends.length === 0) return;
    setSubmitting(true);
    setError(null);
    try {
      const res = await api.bookMultiplayer(
        selectedTier,
        selectedFriends,
        undefined,
        selectedTrack || selectedCar
          ? {
              game: "assetto_corsa",
              game_mode: "practice",
              track: selectedTrack || "",
              car: selectedCar || "",
              difficulty: "medium",
              transmission: "auto",
            }
          : undefined
      );
      if (res.error) {
        setError(res.error);
        setSubmitting(false);
        return;
      }
      // Success -- redirect to group session page
      router.push("/book/group");
    } catch {
      setError("Failed to create session");
      setSubmitting(false);
    }
  };

  // ─── Loading ─────────────────────────────────────────────────────────────

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  // ─── Render ──────────────────────────────────────────────────────────────

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        {/* Back button */}
        <button
          onClick={() => (step > 0 ? setStep(step - 1) : router.push("/book"))}
          className="text-rp-red text-sm mb-4 flex items-center gap-1"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            className="w-4 h-4"
          >
            <path
              d="M19 12H5M12 19l-7-7 7-7"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          {step > 0 ? "Back" : "Back to Booking"}
        </button>

        {/* Header */}
        <h1 className="text-2xl font-bold text-white mb-1">
          Create Multiplayer Session
        </h1>
        <p className="text-rp-grey text-sm mb-6">
          Race against your friends on LAN
        </p>

        {/* Step indicator */}
        <div className="flex gap-2 mb-8">
          {STEPS.map((label, i) => (
            <div key={label} className="flex-1">
              <div
                className={`h-1 rounded-full mb-1 ${
                  i <= step ? "bg-rp-red" : "bg-neutral-700"
                }`}
              />
              <p
                className={`text-[10px] ${
                  i === step
                    ? "text-white font-medium"
                    : i < step
                    ? "text-rp-grey"
                    : "text-neutral-600"
                }`}
              >
                {label}
              </p>
            </div>
          ))}
        </div>

        {/* Error */}
        {error && (
          <div className="bg-red-500/10 border border-red-500/30 rounded-xl p-4 mb-4">
            <p className="text-red-400 text-sm">{error}</p>
          </div>
        )}

        {/* ─── Step 1: Select Friends ─────────────────────────────────── */}
        {step === 0 && (
          <div>
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-white font-semibold">Select Friends</h2>
              <span className="text-xs text-rp-grey">
                {selectedFriends.length}/7 selected
              </span>
            </div>

            {onlineFriends.length === 0 ? (
              <div className="bg-rp-card border border-rp-border rounded-xl p-6 text-center">
                <p className="text-rp-grey text-sm mb-2">
                  No friends online right now
                </p>
                <p className="text-neutral-500 text-xs">
                  Friends need to be online and not in a session to be invited
                </p>
                <button
                  onClick={() => router.push("/friends")}
                  className="mt-4 text-rp-red text-sm font-medium"
                >
                  Manage Friends
                </button>
              </div>
            ) : (
              <div className="space-y-2">
                {onlineFriends.map((friend) => {
                  const isSelected = selectedFriends.includes(friend.driver_id);
                  return (
                    <button
                      key={friend.driver_id}
                      onClick={() => toggleFriend(friend.driver_id)}
                      className={`w-full text-left bg-rp-card border rounded-xl p-4 flex items-center gap-3 transition-colors ${
                        isSelected
                          ? "border-rp-red ring-1 ring-rp-red/30"
                          : "border-rp-border"
                      }`}
                    >
                      {/* Avatar */}
                      <div
                        className={`w-10 h-10 rounded-full flex items-center justify-center ${
                          isSelected ? "bg-rp-red/20" : "bg-neutral-700"
                        }`}
                      >
                        <span
                          className={`text-sm font-bold ${
                            isSelected ? "text-rp-red" : "text-neutral-300"
                          }`}
                        >
                          {friend.name.charAt(0).toUpperCase()}
                        </span>
                      </div>

                      {/* Info */}
                      <div className="flex-1 min-w-0">
                        <p className="text-white text-sm font-medium truncate">
                          {friend.name}
                        </p>
                        {friend.customer_id && (
                          <p className="text-rp-grey text-xs truncate">
                            {friend.customer_id}
                          </p>
                        )}
                      </div>

                      {/* Online dot + Check */}
                      <div className="flex items-center gap-2">
                        <div className="w-2 h-2 bg-emerald-500 rounded-full" />
                        {isSelected && (
                          <svg
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth={2.5}
                            className="w-5 h-5 text-rp-red"
                          >
                            <path
                              d="M5 13l4 4L19 7"
                              strokeLinecap="round"
                              strokeLinejoin="round"
                            />
                          </svg>
                        )}
                      </div>
                    </button>
                  );
                })}

                {/* Show offline friends section */}
                {friends.length > onlineFriends.length && (
                  <p className="text-neutral-500 text-xs text-center pt-2">
                    {friends.length - onlineFriends.length} friend
                    {friends.length - onlineFriends.length !== 1 ? "s" : ""}{" "}
                    offline
                  </p>
                )}
              </div>
            )}

            {/* Next button */}
            <button
              onClick={() => setStep(1)}
              disabled={selectedFriends.length === 0}
              className="w-full mt-6 bg-rp-red hover:bg-rp-red/90 text-white font-semibold py-3 rounded-xl disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              Next: Configure Session
            </button>
          </div>
        )}

        {/* ─── Step 2: Configure Session ──────────────────────────────── */}
        {step === 1 && (
          <div>
            {/* Pricing tier selection */}
            <h2 className="text-white font-semibold mb-3">Select Duration</h2>
            <div className="space-y-2 mb-6">
              {tiers.map((tier) => {
                const isSelected = selectedTier === tier.id;
                const pricePerPerson = tier.price_paise / 100;
                return (
                  <button
                    key={tier.id}
                    onClick={() => setSelectedTier(tier.id)}
                    className={`w-full text-left bg-rp-card border rounded-xl p-4 flex items-center justify-between transition-colors ${
                      isSelected
                        ? "border-rp-red ring-1 ring-rp-red/30"
                        : "border-rp-border"
                    }`}
                  >
                    <div>
                      <p className="text-white font-medium text-sm">
                        {tier.name}
                      </p>
                      <p className="text-rp-grey text-xs">
                        {tier.duration_minutes} minutes
                      </p>
                    </div>
                    <div className="text-right">
                      <p className="text-white font-bold">
                        {pricePerPerson.toFixed(0)} credits
                      </p>
                      <p className="text-rp-grey text-xs">per person</p>
                    </div>
                  </button>
                );
              })}
            </div>

            {/* Track selection */}
            <h2 className="text-white font-semibold mb-3">
              Choose Track{" "}
              <span className="text-rp-grey text-xs font-normal">
                (optional)
              </span>
            </h2>
            <div className="grid grid-cols-2 gap-2 mb-6">
              <button
                onClick={() => setSelectedTrack(null)}
                className={`bg-rp-card border rounded-xl p-3 text-center transition-colors ${
                  !selectedTrack
                    ? "border-rp-red ring-1 ring-rp-red/30"
                    : "border-rp-border"
                }`}
              >
                <p className="text-white text-sm font-medium">Any Track</p>
                <p className="text-rp-grey text-[10px]">Staff picks</p>
              </button>
              {catalog?.tracks.featured.slice(0, 7).map((track: CatalogTrack) => (
                <button
                  key={track.id}
                  onClick={() => setSelectedTrack(track.id)}
                  className={`bg-rp-card border rounded-xl p-3 text-center transition-colors ${
                    selectedTrack === track.id
                      ? "border-rp-red ring-1 ring-rp-red/30"
                      : "border-rp-border"
                  }`}
                >
                  <p className="text-white text-sm font-medium truncate">
                    {track.name}
                  </p>
                  {track.country && (
                    <p className="text-rp-grey text-[10px] truncate">
                      {track.country}
                    </p>
                  )}
                </button>
              ))}
            </div>

            {/* Car selection */}
            <h2 className="text-white font-semibold mb-3">
              Choose Car{" "}
              <span className="text-rp-grey text-xs font-normal">
                (optional)
              </span>
            </h2>
            <div className="grid grid-cols-2 gap-2 mb-6">
              <button
                onClick={() => setSelectedCar(null)}
                className={`bg-rp-card border rounded-xl p-3 text-center transition-colors ${
                  !selectedCar
                    ? "border-rp-red ring-1 ring-rp-red/30"
                    : "border-rp-border"
                }`}
              >
                <p className="text-white text-sm font-medium">Any Car</p>
                <p className="text-rp-grey text-[10px]">Staff picks</p>
              </button>
              {catalog?.cars.featured.slice(0, 7).map((car: CatalogCar) => (
                <button
                  key={car.id}
                  onClick={() => setSelectedCar(car.id)}
                  className={`bg-rp-card border rounded-xl p-3 text-center transition-colors ${
                    selectedCar === car.id
                      ? "border-rp-red ring-1 ring-rp-red/30"
                      : "border-rp-border"
                  }`}
                >
                  <p className="text-white text-sm font-medium truncate">
                    {car.name}
                  </p>
                  <p className="text-rp-grey text-[10px] truncate">
                    {car.category}
                  </p>
                </button>
              ))}
            </div>

            {/* Summary */}
            {selectedTier && (
              <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-4">
                <p className="text-rp-grey text-xs mb-2 uppercase tracking-wider">
                  Session Summary
                </p>
                <div className="space-y-1.5">
                  <SummaryRow
                    label="Players"
                    value={`${selectedFriends.length + 1} (you + ${selectedFriends.length} friend${selectedFriends.length !== 1 ? "s" : ""})`}
                  />
                  <SummaryRow
                    label="Duration"
                    value={selectedTierObj ? `${selectedTierObj.duration_minutes} min` : ""}
                  />
                  <SummaryRow
                    label="Cost per person"
                    value={selectedTierObj ? `${(selectedTierObj.price_paise / 100).toFixed(0)} credits` : ""}
                  />
                  <SummaryRow
                    label="Track"
                    value={
                      selectedTrack
                        ? catalog?.tracks.featured.find(
                            (t: CatalogTrack) => t.id === selectedTrack
                          )?.name || selectedTrack
                        : "Staff picks"
                    }
                  />
                  <SummaryRow
                    label="Car"
                    value={
                      selectedCar
                        ? catalog?.cars.featured.find(
                            (c: CatalogCar) => c.id === selectedCar
                          )?.name || selectedCar
                        : "Staff picks"
                    }
                  />
                </div>
              </div>
            )}

            {/* Create button */}
            <button
              onClick={() => setStep(2)}
              disabled={!selectedTier}
              className="w-full mt-2 bg-rp-red hover:bg-rp-red/90 text-white font-semibold py-3 rounded-xl disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              Create Session
            </button>
          </div>
        )}

        {/* ─── Step 3: Confirm & Submit ───────────────────────────────── */}
        {step === 2 && (
          <div>
            <div className="bg-rp-card border border-rp-border rounded-xl p-5 mb-6">
              <h2 className="text-white font-semibold mb-4">
                Confirm Multiplayer Session
              </h2>
              <div className="space-y-3">
                {/* Friends */}
                <div>
                  <p className="text-rp-grey text-xs mb-2 uppercase tracking-wider">
                    Players ({selectedFriends.length + 1})
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <span className="bg-rp-red/10 text-rp-red text-xs font-medium px-3 py-1.5 rounded-full">
                      You (Host)
                    </span>
                    {selectedFriends.map((fid) => {
                      const friend = friends.find(
                        (f) => f.driver_id === fid
                      );
                      return (
                        <span
                          key={fid}
                          className="bg-neutral-800 text-neutral-300 text-xs px-3 py-1.5 rounded-full"
                        >
                          {friend?.name || "Friend"}
                        </span>
                      );
                    })}
                  </div>
                </div>

                {/* Duration & Cost */}
                {selectedTierObj && (
                  <div className="grid grid-cols-2 gap-3 pt-2">
                    <div className="bg-[#1A1A1A] rounded-lg p-3">
                      <p className="text-rp-grey text-xs">Duration</p>
                      <p className="text-white font-bold">
                        {selectedTierObj.duration_minutes} min
                      </p>
                    </div>
                    <div className="bg-[#1A1A1A] rounded-lg p-3">
                      <p className="text-rp-grey text-xs">Cost / person</p>
                      <p className="text-white font-bold">
                        {(selectedTierObj.price_paise / 100).toFixed(0)} credits
                      </p>
                    </div>
                  </div>
                )}

                {/* Track & Car */}
                <div className="grid grid-cols-2 gap-3">
                  <div className="bg-[#1A1A1A] rounded-lg p-3">
                    <p className="text-rp-grey text-xs">Track</p>
                    <p className="text-white text-sm font-medium truncate">
                      {selectedTrack
                        ? catalog?.tracks.featured.find(
                            (t: CatalogTrack) => t.id === selectedTrack
                          )?.name || selectedTrack
                        : "Staff picks"}
                    </p>
                  </div>
                  <div className="bg-[#1A1A1A] rounded-lg p-3">
                    <p className="text-rp-grey text-xs">Car</p>
                    <p className="text-white text-sm font-medium truncate">
                      {selectedCar
                        ? catalog?.cars.featured.find(
                            (c: CatalogCar) => c.id === selectedCar
                          )?.name || selectedCar
                        : "Staff picks"}
                    </p>
                  </div>
                </div>
              </div>
            </div>

            {/* Info note */}
            <div className="bg-amber-500/10 border border-amber-500/20 rounded-xl p-4 mb-6">
              <p className="text-amber-400 text-xs">
                Each player will be charged from their wallet when they accept
                the invite. Make sure everyone has enough credits.
              </p>
            </div>

            {/* Submit */}
            <button
              onClick={handleSubmit}
              disabled={submitting}
              className="w-full bg-rp-red hover:bg-rp-red/90 text-white font-semibold py-3 rounded-xl disabled:opacity-50 transition-colors flex items-center justify-center gap-2"
            >
              {submitting ? (
                <>
                  <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                  Creating Session...
                </>
              ) : (
                "Confirm & Send Invites"
              )}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// ─── Sub-components ──────────────────────────────────────────────────────────

function SummaryRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between items-center">
      <span className="text-rp-grey text-sm">{label}</span>
      <span className="text-neutral-200 text-sm">{value}</span>
    </div>
  );
}
