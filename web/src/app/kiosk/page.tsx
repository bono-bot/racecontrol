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
    <div className="min-h-screen bg-zinc-950 flex items-center justify-center p-8">
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-orange-500 mb-2">RacingPoint</h1>
          <p className="text-zinc-500">Driver Check-In</p>
        </div>

        {registered ? (
          <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-xl p-8 text-center">
            <div className="text-4xl mb-4">&#9989;</div>
            <h2 className="text-xl font-bold text-emerald-400 mb-2">
              Welcome, {registered}!
            </h2>
            <p className="text-zinc-400 text-sm mb-6">
              You&apos;re registered and ready to race.
            </p>
            <button
              onClick={() => setRegistered(null)}
              className="px-6 py-2 bg-zinc-800 text-zinc-300 rounded-lg hover:bg-zinc-700 transition-colors text-sm"
            >
              Register Another Driver
            </button>
          </div>
        ) : (
          <form onSubmit={handleRegister} className="space-y-4">
            <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-6 space-y-4">
              <div>
                <label className="block text-sm text-zinc-400 mb-1.5">Name</label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                  placeholder="Enter your name"
                  className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-4 py-3 text-zinc-100 placeholder:text-zinc-600 focus:outline-none focus:border-orange-500 transition-colors"
                />
              </div>
              <div>
                <label className="block text-sm text-zinc-400 mb-1.5">
                  Email <span className="text-zinc-600">(optional)</span>
                </label>
                <input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="your@email.com"
                  className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-4 py-3 text-zinc-100 placeholder:text-zinc-600 focus:outline-none focus:border-orange-500 transition-colors"
                />
              </div>
            </div>

            {error && (
              <p className="text-red-400 text-sm text-center">{error}</p>
            )}

            <button
              type="submit"
              disabled={!name.trim()}
              className="w-full py-3 bg-orange-500 text-white font-bold rounded-lg hover:bg-orange-600 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
              Check In
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
