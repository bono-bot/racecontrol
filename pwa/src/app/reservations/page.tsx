"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { RemoteReservation, Experience, PricingTier } from "@/lib/api";

// ─── Status badge colors ─────────────────────────────────────────────────

const STATUS_STYLES: Record<string, { bg: string; text: string; label: string }> = {
  pending_debit: { bg: "bg-yellow-900/30", text: "text-yellow-400", label: "Processing" },
  confirmed: { bg: "bg-emerald-900/30", text: "text-emerald-400", label: "Confirmed" },
  expired: { bg: "bg-red-900/30", text: "text-red-400", label: "Expired" },
  cancelled: { bg: "bg-neutral-800", text: "text-neutral-400", label: "Cancelled" },
  failed: { bg: "bg-red-900/30", text: "text-red-400", label: "Failed" },
};

function getStatusStyle(status: string) {
  return STATUS_STYLES[status] || { bg: "bg-neutral-800", text: "text-neutral-400", label: status };
}

// ─── Main ────────────────────────────────────────────────────────────────

export default function ReservationsPage() {
  const router = useRouter();
  const [hydrated, setHydrated] = useState(false);
  const [reservation, setReservation] = useState<RemoteReservation | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Cancel state
  const [cancelling, setCancelling] = useState(false);
  const [showCancelConfirm, setShowCancelConfirm] = useState(false);
  const [cancelResult, setCancelResult] = useState<{ refund_paise: number } | null>(null);

  // Modify state
  const [showModify, setShowModify] = useState(false);
  const [modifying, setModifying] = useState(false);
  const [experiences, setExperiences] = useState<Experience[]>([]);
  const [pricingTiers, setPricingTiers] = useState<PricingTier[]>([]);
  const [selectedExperience, setSelectedExperience] = useState("");
  const [selectedTier, setSelectedTier] = useState("");

  // Copy PIN state
  const [copied, setCopied] = useState(false);

  // Auth + hydration check
  useEffect(() => {
    setHydrated(true);
    if (!isLoggedIn()) {
      router.replace("/login");
    }
  }, [router]);

  // Fetch reservation on mount
  useEffect(() => {
    if (!hydrated) return;
    if (!isLoggedIn()) return;

    async function load() {
      try {
        const res = await api.getReservation();
        if (res.error) {
          setError(res.error);
        } else {
          setReservation(res.reservation || null);
        }
      } catch {
        setError("Failed to load reservation");
      } finally {
        setLoading(false);
      }
    }
    load();
  }, [hydrated]);

  async function handleCancel() {
    setCancelling(true);
    setError(null);
    try {
      const res = await api.cancelReservation();
      if (res.status === "cancelled") {
        setCancelResult({ refund_paise: res.refund_paise || 0 });
        setReservation(null);
        setShowCancelConfirm(false);
      } else {
        setError(res.error || "Cancel failed");
      }
    } catch {
      setError("Network error");
    } finally {
      setCancelling(false);
    }
  }

  async function handleModify() {
    if (!selectedExperience || !selectedTier) return;
    setModifying(true);
    setError(null);
    try {
      const res = await api.modifyReservation(selectedExperience, selectedTier);
      if (res.pin) {
        // Refresh reservation
        const updated = await api.getReservation();
        setReservation(updated.reservation || null);
        setShowModify(false);
      } else {
        setError(res.error || "Modify failed");
      }
    } catch {
      setError("Network error");
    } finally {
      setModifying(false);
    }
  }

  async function openModify() {
    setShowModify(true);
    setError(null);
    try {
      const res = await api.experiences();
      if (res.experiences) setExperiences(res.experiences);
      if (res.pricing_tiers) setPricingTiers(res.pricing_tiers.filter((t) => !t.is_trial));
    } catch {
      setError("Failed to load experiences");
    }
  }

  function handleCopy(pin: string) {
    navigator.clipboard.writeText(pin).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }).catch(() => {
      // Fallback silently
    });
  }

  if (!hydrated || loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  const statusStyle = reservation ? getStatusStyle(reservation.status) : null;
  const expiresDate = reservation?.expires_at ? new Date(reservation.expires_at) : null;
  const createdDate = reservation?.created_at ? new Date(reservation.created_at) : null;
  const canCancel = reservation && (reservation.status === "pending_debit" || reservation.status === "confirmed");
  const canModify = reservation && (reservation.status === "pending_debit" || reservation.status === "confirmed");

  return (
    <div className="min-h-screen pb-24">
      {/* Header */}
      <div className="px-4 pt-6 pb-4">
        <div className="flex items-center gap-3 mb-2">
          <button
            onClick={() => router.push("/dashboard")}
            className="w-10 h-10 flex items-center justify-center rounded-xl bg-rp-card border border-rp-border"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-white">
              <path d="M15 18l-6-6 6-6" />
            </svg>
          </button>
          <h1 className="text-xl font-bold text-white">My Reservation</h1>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="mx-4 mb-4 bg-red-900/30 border border-red-500/30 rounded-xl p-3 text-red-400 text-sm">
          {error}
          <button onClick={() => setError(null)} className="ml-2 underline">Dismiss</button>
        </div>
      )}

      {/* Cancel success */}
      {cancelResult && (
        <div className="mx-4 mb-4 bg-emerald-900/30 border border-emerald-500/30 rounded-xl p-4 text-center">
          <p className="text-emerald-400 font-semibold mb-1">Reservation Cancelled</p>
          {cancelResult.refund_paise > 0 && (
            <p className="text-emerald-300 text-sm">
              {(cancelResult.refund_paise / 100).toFixed(0)} credits refunded to your wallet
            </p>
          )}
          <a href="/book" className="inline-block mt-3 text-rp-red text-sm font-medium underline">
            Book again
          </a>
        </div>
      )}

      <div className="px-4">
        {/* No active reservation */}
        {!reservation && !cancelResult && (
          <div className="flex flex-col items-center justify-center py-20 text-center">
            <div className="w-16 h-16 rounded-full bg-neutral-800 flex items-center justify-center mb-4">
              <svg className="w-8 h-8 text-rp-grey" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
              </svg>
            </div>
            <h2 className="text-lg font-semibold text-white mb-2">No Active Reservation</h2>
            <p className="text-rp-grey text-sm mb-6">Book a session to get your race PIN</p>
            <a
              href="/book"
              className="bg-rp-red text-white font-semibold py-3 px-8 rounded-xl text-base"
            >
              Book Now
            </a>
          </div>
        )}

        {/* Active reservation card */}
        {reservation && (
          <div className="space-y-4">
            {/* PIN display */}
            <div className="bg-rp-card border border-rp-border rounded-xl p-6 text-center">
              <p className="text-rp-grey text-xs uppercase tracking-wide mb-3">Your PIN</p>
              <div className="flex gap-3 justify-center mb-3">
                {reservation.pin.split("").map((digit, i) => (
                  <div
                    key={i}
                    className="w-14 h-18 bg-rp-dark border-2 border-[#E10600] rounded-xl flex items-center justify-center py-3"
                  >
                    <span className="text-3xl font-bold tracking-widest text-[#E10600]">{digit}</span>
                  </div>
                ))}
              </div>
              <button
                onClick={() => handleCopy(reservation.pin)}
                className="flex items-center gap-2 mx-auto text-sm text-rp-grey hover:text-white transition-colors"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" strokeWidth="2" />
                  <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" strokeWidth="2" />
                </svg>
                {copied ? "Copied!" : "Copy PIN"}
              </button>
            </div>

            {/* Reservation details */}
            <div className="bg-rp-card border border-rp-border rounded-xl divide-y divide-rp-border">
              <div className="flex items-center justify-between px-4 py-3">
                <span className="text-rp-grey text-sm">Status</span>
                {statusStyle && (
                  <span className={`text-xs font-semibold px-2.5 py-1 rounded-full ${statusStyle.bg} ${statusStyle.text}`}>
                    {statusStyle.label}
                  </span>
                )}
              </div>
              <div className="flex items-center justify-between px-4 py-3">
                <span className="text-rp-grey text-sm">Experience</span>
                <span className="text-white text-sm font-medium">{reservation.experience_name}</span>
              </div>
              <div className="flex items-center justify-between px-4 py-3">
                <span className="text-rp-grey text-sm">Cost</span>
                <span className="text-white text-sm font-bold">
                  {(reservation.price_paise / 100).toFixed(0)} credits
                </span>
              </div>
              <div className="flex items-center justify-between px-4 py-3">
                <span className="text-rp-grey text-sm">Expires</span>
                <span className="text-white text-sm font-medium">
                  {expiresDate
                    ? expiresDate.toLocaleString("en-IN", {
                        timeZone: "Asia/Kolkata",
                        dateStyle: "medium",
                        timeStyle: "short",
                      })
                    : "\u2014"}
                </span>
              </div>
              <div className="flex items-center justify-between px-4 py-3">
                <span className="text-rp-grey text-sm">Booked</span>
                <span className="text-white text-sm font-medium">
                  {createdDate
                    ? createdDate.toLocaleString("en-IN", {
                        timeZone: "Asia/Kolkata",
                        dateStyle: "medium",
                        timeStyle: "short",
                      })
                    : "\u2014"}
                </span>
              </div>
            </div>

            {/* Actions */}
            <div className="flex gap-3">
              {canModify && (
                <button
                  onClick={openModify}
                  className="flex-1 bg-rp-card border border-rp-border text-white font-semibold py-3 rounded-xl text-base hover:border-rp-red/50 transition-colors"
                >
                  Change Booking
                </button>
              )}
              {canCancel && (
                <button
                  onClick={() => setShowCancelConfirm(true)}
                  className="flex-1 bg-red-900/20 border border-red-500/30 text-red-400 font-semibold py-3 rounded-xl text-base hover:bg-red-900/30 transition-colors"
                >
                  Cancel
                </button>
              )}
            </div>

            {/* Cancel confirmation dialog */}
            {showCancelConfirm && (
              <div className="bg-rp-card border border-red-500/30 rounded-xl p-5">
                <p className="text-white font-semibold mb-2">Cancel Reservation?</p>
                <p className="text-rp-grey text-sm mb-4">
                  Your credits will be refunded to your wallet.
                </p>
                <div className="flex gap-3">
                  <button
                    onClick={() => setShowCancelConfirm(false)}
                    className="flex-1 bg-rp-dark border border-rp-border text-white py-2.5 rounded-xl text-sm font-medium"
                  >
                    Keep Reservation
                  </button>
                  <button
                    onClick={handleCancel}
                    disabled={cancelling}
                    className="flex-1 bg-red-600 text-white py-2.5 rounded-xl text-sm font-semibold disabled:opacity-50"
                  >
                    {cancelling ? "Cancelling..." : "Yes, Cancel"}
                  </button>
                </div>
              </div>
            )}

            {/* Modify form (inline) */}
            {showModify && (
              <div className="bg-rp-card border border-rp-border rounded-xl p-5">
                <p className="text-white font-semibold mb-3">Change Your Booking</p>

                <div className="space-y-3">
                  <div>
                    <label className="text-rp-grey text-xs uppercase tracking-wide mb-1 block">
                      Experience
                    </label>
                    <select
                      value={selectedExperience}
                      onChange={(e) => setSelectedExperience(e.target.value)}
                      className="w-full bg-rp-dark border border-rp-border rounded-lg px-3 py-2.5 text-sm text-white outline-none focus:border-rp-red/50"
                    >
                      <option value="">Select experience</option>
                      {experiences.map((exp) => (
                        <option key={exp.id} value={exp.id}>
                          {exp.name} ({exp.car} at {exp.track})
                        </option>
                      ))}
                    </select>
                  </div>

                  <div>
                    <label className="text-rp-grey text-xs uppercase tracking-wide mb-1 block">
                      Duration
                    </label>
                    <select
                      value={selectedTier}
                      onChange={(e) => setSelectedTier(e.target.value)}
                      className="w-full bg-rp-dark border border-rp-border rounded-lg px-3 py-2.5 text-sm text-white outline-none focus:border-rp-red/50"
                    >
                      <option value="">Select duration</option>
                      {pricingTiers.map((t) => (
                        <option key={t.id} value={t.id}>
                          {t.name} - {t.duration_minutes} min ({(t.price_paise / 100).toFixed(0)} credits)
                        </option>
                      ))}
                    </select>
                  </div>
                </div>

                <div className="flex gap-3 mt-4">
                  <button
                    onClick={() => setShowModify(false)}
                    className="flex-1 bg-rp-dark border border-rp-border text-white py-2.5 rounded-xl text-sm font-medium"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleModify}
                    disabled={modifying || !selectedExperience || !selectedTier}
                    className="flex-1 bg-rp-red text-white py-2.5 rounded-xl text-sm font-semibold disabled:opacity-50"
                  >
                    {modifying ? "Updating..." : "Confirm Change"}
                  </button>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
