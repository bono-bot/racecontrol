"use client";

import { useState, useEffect } from "react";
import { api } from "@/lib/api";

export default function VenueRegisterPage() {
  const [name, setName] = useState("");
  const [dob, setDob] = useState("");
  const [guardianName, setGuardianName] = useState("");
  const [waiverConsent, setWaiverConsent] = useState(false);
  const [isMinor, setIsMinor] = useState(false);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<{ name: string; customer_id: string } | null>(null);

  useEffect(() => {
    if (!dob) { setIsMinor(false); return; }
    const birthDate = new Date(dob);
    const today = new Date();
    let age = today.getFullYear() - birthDate.getFullYear();
    const m = today.getMonth() - birthDate.getMonth();
    if (m < 0 || (m === 0 && today.getDate() < birthDate.getDate())) age--;
    setIsMinor(age < 18);
  }, [dob]);

  async function handleSubmit() {
    if (name.trim().length < 2) { setError("Name must be at least 2 characters"); return; }
    if (!dob) { setError("Date of birth is required"); return; }
    if (!waiverConsent) { setError("You must accept the safety waiver"); return; }
    if (isMinor && !guardianName.trim()) { setError("Guardian name is required for under 18"); return; }

    setLoading(true);
    setError(null);

    try {
      const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://192.168.31.23:8080";
      const res = await fetch(`${API_BASE}/api/v1/venue/register`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name: name.trim(),
          dob,
          waiver_consent: waiverConsent,
          guardian_name: isMinor ? guardianName.trim() : undefined,
        }),
      });

      const data = await res.json();

      if (data.error) {
        setError(data.error);
      } else {
        setSuccess({ name: data.name || name.trim(), customer_id: data.customer_id || "" });
      }
    } catch {
      setError("Network error. Please try again.");
    } finally {
      setLoading(false);
    }
  }

  function handleReset() {
    setName(""); setDob(""); setGuardianName(""); setWaiverConsent(false);
    setError(null); setSuccess(null); setIsMinor(false);
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-rp-black px-4">
      <div className="w-full max-w-md">
        {/* Logo */}
        <div className="text-center mb-8">
          <h1 className="text-4xl font-black tracking-tight">
            <span className="text-rp-red">Racing</span>
            <span className="text-white">Point</span>
          </h1>
          <p className="text-rp-grey text-sm mt-2 tracking-widest uppercase">
            Quick Registration
          </p>
        </div>

        {success ? (
          <div className="bg-rp-card border border-rp-border rounded-2xl p-8 text-center">
            <div className="w-16 h-16 rounded-full bg-green-500/20 flex items-center justify-center mx-auto mb-4">
              <svg className="w-8 h-8 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <h2 className="text-xl font-bold text-white mb-2">Welcome, {success.name}!</h2>
            <p className="text-neutral-400 text-sm mb-4">
              Your customer ID is:
            </p>
            <p className="text-3xl font-bold text-rp-red font-mono tracking-wider mb-6">
              {success.customer_id}
            </p>
            <p className="text-neutral-500 text-xs mb-6">
              Give this ID to the staff at the counter to get started
            </p>
            <button
              onClick={handleReset}
              className="w-full py-3 bg-rp-surface border border-rp-border rounded-xl text-neutral-300 font-medium text-sm hover:text-white transition-colors"
            >
              Register Another Person
            </button>
          </div>
        ) : (
          <div className="bg-rp-card border border-rp-border rounded-2xl p-6">
            <h2 className="text-lg font-bold text-white mb-1">Register</h2>
            <p className="text-neutral-500 text-sm mb-6">Fill in your details to get started</p>

            <div className="space-y-4">
              <div>
                <label className="block text-xs text-neutral-400 mb-1">Full Name *</label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Your name"
                  className="w-full bg-rp-surface border border-rp-border rounded-xl px-4 py-3 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors"
                  autoFocus
                />
              </div>

              <div>
                <label className="block text-xs text-neutral-400 mb-1">Date of Birth *</label>
                <input
                  type="date"
                  value={dob}
                  onChange={(e) => setDob(e.target.value)}
                  className="w-full bg-rp-surface border border-rp-border rounded-xl px-4 py-3 text-white focus:outline-none focus:border-rp-red transition-colors"
                />
              </div>

              {isMinor && (
                <div>
                  <label className="block text-xs text-neutral-400 mb-1">Guardian Name *</label>
                  <input
                    type="text"
                    value={guardianName}
                    onChange={(e) => setGuardianName(e.target.value)}
                    placeholder="Parent or guardian name"
                    className="w-full bg-rp-surface border border-rp-border rounded-xl px-4 py-3 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors"
                  />
                  <p className="text-xs text-amber-400 mt-1">Under 18 — guardian name required</p>
                </div>
              )}

              <label className="flex items-start gap-3 cursor-pointer pt-2">
                <input
                  type="checkbox"
                  checked={waiverConsent}
                  onChange={(e) => setWaiverConsent(e.target.checked)}
                  className="mt-1 w-5 h-5 rounded border-rp-border accent-rp-red"
                />
                <span className="text-neutral-300 text-sm">
                  I accept the safety waiver and understand the risks involved in sim racing
                  {isMinor && " (signed by guardian on behalf of minor)"}
                </span>
              </label>

              {error && <p className="text-red-400 text-sm">{error}</p>}

              <button
                onClick={handleSubmit}
                disabled={loading}
                className="w-full py-3 bg-rp-red text-white font-semibold rounded-xl disabled:opacity-50 text-sm transition-colors"
              >
                {loading ? "Registering..." : "Register"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
