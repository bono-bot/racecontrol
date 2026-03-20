"use client";
import { useState, useEffect, useRef, FormEvent } from "react";
import { useRouter } from "next/navigation";
import { setToken, isAuthenticated } from "@/lib/auth";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

export default function LoginPage() {
  const [pin, setPin] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const router = useRouter();

  useEffect(() => {
    if (isAuthenticated()) {
      router.push("/");
      return;
    }
    inputRef.current?.focus();
  }, [router]);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);

    try {
      const res = await fetch(`${API_BASE}/api/v1/auth/admin-login`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ pin }),
      });

      if (res.status === 200) {
        const data = await res.json();
        setToken(data.token);
        router.push("/");
      } else if (res.status === 401) {
        setError("Invalid PIN");
        setPin("");
        inputRef.current?.focus();
      } else if (res.status === 503) {
        setError("Admin PIN not configured");
      } else {
        setError("Login failed. Please try again.");
      }
    } catch {
      setError("Cannot reach server. Check your connection.");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-[#1A1A1A] px-4">
      <div className="w-full max-w-sm bg-[#222222] border border-[#333333] rounded-xl p-8 shadow-2xl">
        {/* Header */}
        <div className="text-center mb-8">
          <h1 className="text-2xl font-bold text-white tracking-tight">
            RaceControl
          </h1>
          <p className="text-sm text-neutral-400 mt-1">Staff Access</p>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label
              htmlFor="pin"
              className="block text-xs font-medium text-neutral-400 mb-1.5"
            >
              Enter PIN
            </label>
            <input
              ref={inputRef}
              id="pin"
              type="password"
              inputMode="numeric"
              maxLength={6}
              value={pin}
              onChange={(e) => setPin(e.target.value.replace(/\D/g, ""))}
              placeholder="------"
              className="w-full px-4 py-3 bg-[#1A1A1A] border border-[#333333] rounded-lg text-white text-center text-2xl tracking-[0.5em] placeholder:text-neutral-600 focus:outline-none focus:border-[#E10600] focus:ring-1 focus:ring-[#E10600] transition-colors"
              disabled={loading}
              autoComplete="off"
            />
          </div>

          {error && (
            <p className="text-sm text-[#E10600] text-center font-medium">
              {error}
            </p>
          )}

          <button
            type="submit"
            disabled={loading || pin.length === 0}
            className="w-full py-3 bg-[#E10600] hover:bg-[#C00500] disabled:bg-neutral-700 disabled:text-neutral-500 text-white font-semibold rounded-lg transition-colors"
          >
            {loading ? "Verifying..." : "Unlock"}
          </button>
        </form>
      </div>
    </div>
  );
}
