"use client";

import { useState } from "react";
import { api } from "@/lib/api";

export default function KioskPage() {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [registered, setRegistered] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleRegister(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      const res = await api.createDriver({ name, email: email || undefined });
      setRegistered(res.name);
      setName("");
      setEmail("");
    } catch {
      setError("Registration failed. Please try again.");
    }
  }

  return (
    <div className="min-h-screen bg-rp-black flex items-center justify-center p-8">
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-rp-red mb-2">RacingPoint</h1>
          <p className="text-rp-grey">Driver Check-In</p>
        </div>

        {registered ? (
          <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-xl p-8 text-center">
            <div className="text-4xl mb-4">&#9989;</div>
            <h2 className="text-xl font-bold text-emerald-400 mb-2">
              Welcome, {registered}!
            </h2>
            <p className="text-neutral-400 text-sm mb-6">
              You&apos;re registered and ready to race.
            </p>
            <button
              onClick={() => setRegistered(null)}
              className="px-6 py-2 bg-rp-card text-neutral-300 rounded-lg hover:bg-rp-card transition-colors text-sm"
            >
              Register Another Driver
            </button>
          </div>
        ) : (
          <form onSubmit={handleRegister} className="space-y-4">
            <div className="bg-rp-card border border-rp-border rounded-xl p-6 space-y-4">
              <div>
                <label className="block text-sm text-neutral-400 mb-1.5">Name</label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                  placeholder="Enter your name"
                  className="w-full bg-rp-card border border-rp-border rounded-lg px-4 py-3 text-white placeholder:text-rp-grey focus:outline-none focus:border-rp-red transition-colors"
                />
              </div>
              <div>
                <label className="block text-sm text-neutral-400 mb-1.5">
                  Email <span className="text-rp-grey">(optional)</span>
                </label>
                <input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="your@email.com"
                  className="w-full bg-rp-card border border-rp-border rounded-lg px-4 py-3 text-white placeholder:text-rp-grey focus:outline-none focus:border-rp-red transition-colors"
                />
              </div>
            </div>

            {error && (
              <p className="text-red-400 text-sm text-center">{error}</p>
            )}

            <button
              type="submit"
              disabled={!name.trim()}
              className="w-full py-3 bg-rp-red text-white font-bold rounded-lg hover:bg-rp-red disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
              Check In
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
