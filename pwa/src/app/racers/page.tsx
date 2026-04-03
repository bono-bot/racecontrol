"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { Racer } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

export default function RacersPage() {
  const router = useRouter();
  const [racers, setRacers] = useState<Racer[]>([]);
  const [maxRacers, setMaxRacers] = useState(3);
  const [loading, setLoading] = useState(true);

  const [showAdd, setShowAdd] = useState(false);
  const [addName, setAddName] = useState("");
  const [addDob, setAddDob] = useState("");
  const [addWaiver, setAddWaiver] = useState(false);
  const [addSubmitting, setAddSubmitting] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);

  useEffect(() => {
    if (!isLoggedIn()) { router.replace("/login"); return; }
    loadRacers();
  }, [router]);

  async function loadRacers() {
    try {
      const res = await api.listRacers();
      setRacers(res.racers || []);
      setMaxRacers(res.max_racers || 3);
    } catch { /* silent */ }
    setLoading(false);
  }

  async function handleAdd() {
    if (addName.trim().length < 2) { setAddError("Name must be at least 2 characters"); return; }
    if (!addDob) { setAddError("Date of birth is required"); return; }
    if (!addWaiver) { setAddError("You must accept the safety waiver on their behalf"); return; }
    setAddSubmitting(true);
    setAddError(null);
    try {
      const res = await api.addRacer({ name: addName.trim(), dob: addDob, waiver_consent: true });
      if (res.error) {
        setAddError(res.error);
      } else {
        setShowAdd(false);
        setAddName(""); setAddDob(""); setAddWaiver(false);
        loadRacers();
      }
    } catch {
      setAddError("Network error. Try again.");
    } finally {
      setAddSubmitting(false);
    }
  }

  if (loading) {
    return (
      <div className="min-h-screen pb-20">
        <div className="flex items-center justify-center py-24">
          <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
        </div>
        <BottomNav />
      </div>
    );
  }

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-2xl font-bold text-white">My Racers</h1>
            <p className="text-neutral-400 text-sm">{racers.length} of {maxRacers} racers</p>
          </div>
          {racers.length < maxRacers && (
            <button
              onClick={() => setShowAdd(true)}
              className="bg-rp-red text-white font-semibold px-4 py-2 rounded-xl text-sm"
            >
              Add Racer
            </button>
          )}
        </div>

        {/* Racer list */}
        {racers.length === 0 ? (
          <div className="bg-rp-card border border-rp-border rounded-xl p-6 text-center">
            <p className="text-neutral-400 mb-2">No racers added yet</p>
            <p className="text-neutral-500 text-sm">Add your children or friends so they can race under your account</p>
          </div>
        ) : (
          <div className="space-y-3">
            {racers.map((racer) => (
              <div key={racer.id} className="bg-rp-card border border-rp-border rounded-xl p-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-full bg-rp-red/20 flex items-center justify-center text-rp-red font-bold">
                      {racer.name.charAt(0).toUpperCase()}
                    </div>
                    <div>
                      <p className="text-white font-medium">{racer.name}</p>
                      <p className="text-neutral-500 text-xs">
                        Age {racer.age} {racer.is_minor && " (Minor)"}
                      </p>
                    </div>
                  </div>
                  <div className="text-right">
                    <p className="text-white text-sm font-bold">{racer.total_laps}</p>
                    <p className="text-neutral-500 text-xs">laps</p>
                  </div>
                </div>
                {!racer.has_used_trial && (
                  <div className="mt-2 bg-emerald-900/20 border border-emerald-500/20 rounded-lg px-3 py-1.5 text-center">
                    <span className="text-emerald-400 text-xs font-medium">Free trial available</span>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        {/* Add Racer Modal */}
        {showAdd && (
          <div className="fixed inset-0 z-50 flex items-end justify-center bg-black/70 backdrop-blur-sm">
            <div className="bg-rp-card border-t border-rp-border rounded-t-2xl p-6 w-full max-w-lg animate-slide-up">
              <h2 className="text-lg font-bold text-white mb-4">Add Racer</h2>

              <label className="block text-xs text-neutral-400 mb-1">Name</label>
              <input
                type="text"
                value={addName}
                onChange={(e) => setAddName(e.target.value)}
                placeholder="Racer name"
                className="w-full bg-rp-surface border border-rp-border rounded-xl px-4 py-3 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-3"
                autoFocus
              />

              <label className="block text-xs text-neutral-400 mb-1">Date of Birth</label>
              <input
                type="date"
                value={addDob}
                onChange={(e) => setAddDob(e.target.value)}
                className="w-full bg-rp-surface border border-rp-border rounded-xl px-4 py-3 text-white focus:outline-none focus:border-rp-red transition-colors mb-3"
              />

              <label className="flex items-start gap-3 mb-4 cursor-pointer">
                <input
                  type="checkbox"
                  checked={addWaiver}
                  onChange={(e) => setAddWaiver(e.target.checked)}
                  className="mt-1 w-5 h-5 rounded border-rp-border accent-rp-red"
                />
                <span className="text-neutral-300 text-sm">
                  I accept the safety waiver on behalf of this racer and confirm I am their parent or guardian
                </span>
              </label>

              {addError && <p className="text-red-400 text-xs mb-3">{addError}</p>}

              <div className="flex gap-3">
                <button
                  onClick={() => { setShowAdd(false); setAddError(null); }}
                  className="flex-1 py-3 rounded-xl text-sm font-medium bg-rp-surface text-neutral-400 border border-rp-border"
                >
                  Cancel
                </button>
                <button
                  onClick={handleAdd}
                  disabled={addSubmitting}
                  className="flex-1 py-3 rounded-xl text-sm font-semibold bg-rp-red text-white disabled:opacity-50"
                >
                  {addSubmitting ? "Adding..." : "Add Racer"}
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
      <BottomNav />
    </div>
  );
}
