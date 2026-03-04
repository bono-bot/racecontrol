"use client";

import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";

export default function RegisterPage() {
  const router = useRouter();
  const [name, setName] = useState("");
  const [dob, setDob] = useState("");
  const [email, setEmail] = useState("");
  const [guardianName, setGuardianName] = useState("");
  const [guardianPhone, setGuardianPhone] = useState("");
  const [waiverConsent, setWaiverConsent] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [isMinor, setIsMinor] = useState(false);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
    }
  }, [router]);

  // Check if minor based on DOB
  useEffect(() => {
    if (!dob) {
      setIsMinor(false);
      return;
    }
    const birthDate = new Date(dob);
    const today = new Date();
    let age = today.getFullYear() - birthDate.getFullYear();
    const monthDiff = today.getMonth() - birthDate.getMonth();
    if (monthDiff < 0 || (monthDiff === 0 && today.getDate() < birthDate.getDate())) {
      age--;
    }
    setIsMinor(age < 18);
  }, [dob]);

  const handleSubmit = async () => {
    if (name.trim().length < 2) {
      setError("Name must be at least 2 characters");
      return;
    }
    if (!dob) {
      setError("Date of birth is required");
      return;
    }
    if (!waiverConsent) {
      setError("You must accept the safety waiver");
      return;
    }
    if (isMinor && !guardianName.trim()) {
      setError("Guardian name is required for customers under 18");
      return;
    }

    setLoading(true);
    setError("");

    try {
      const res = await api.register({
        name: name.trim(),
        dob,
        email: email.trim() || undefined,
        waiver_consent: waiverConsent,
        guardian_name: isMinor ? guardianName.trim() : undefined,
        guardian_phone: isMinor ? guardianPhone.trim() || undefined : undefined,
      });

      if (res.error) {
        setError(res.error);
      } else {
        router.replace("/dashboard");
      }
    } catch {
      setError("Network error. Try again.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col items-center justify-center min-h-screen px-6 py-12">
      {/* Logo */}
      <div className="mb-12 text-center">
        <h1 className="text-4xl font-black tracking-tight">
          <span className="text-rp-red">Racing</span>
          <span className="text-white">Point</span>
        </h1>
        <p className="text-rp-grey text-xs mt-2 tracking-widest uppercase">
          Quick Registration
        </p>
      </div>

      <div className="w-full max-w-sm">
        <h2 className="text-2xl font-bold mb-2">Complete Your Profile</h2>
        <p className="text-neutral-400 text-sm mb-8">
          Fill in your details to get started
        </p>

        {/* Name */}
        <label className="block text-sm font-medium text-neutral-400 mb-2">
          Full Name *
        </label>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Your name"
          className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3.5 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-5"
          autoFocus
        />

        {/* Date of Birth */}
        <label className="block text-sm font-medium text-neutral-400 mb-2">
          Date of Birth *
        </label>
        <input
          type="date"
          value={dob}
          onChange={(e) => setDob(e.target.value)}
          max={new Date().toISOString().split("T")[0]}
          className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3.5 text-white focus:outline-none focus:border-rp-red transition-colors mb-5 [color-scheme:dark]"
        />

        {/* Minor warning + guardian fields */}
        {isMinor && (
          <div className="bg-amber-500/10 border border-amber-500/30 rounded-xl p-4 mb-5">
            <p className="text-amber-400 text-sm font-medium mb-3">
              Under 18 — Guardian details required
            </p>
            <label className="block text-sm font-medium text-neutral-400 mb-2">
              Guardian Name *
            </label>
            <input
              type="text"
              value={guardianName}
              onChange={(e) => setGuardianName(e.target.value)}
              placeholder="Parent or guardian name"
              className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-3"
            />
            <label className="block text-sm font-medium text-neutral-400 mb-2">
              Guardian Phone
            </label>
            <input
              type="tel"
              value={guardianPhone}
              onChange={(e) => setGuardianPhone(e.target.value.replace(/\D/g, "").slice(0, 10))}
              placeholder="Phone number"
              className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors"
              inputMode="numeric"
            />
          </div>
        )}

        {/* Email (optional) */}
        <label className="block text-sm font-medium text-neutral-400 mb-2">
          Email <span className="text-zinc-600">(optional)</span>
        </label>
        <input
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder="you@example.com"
          className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3.5 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-6"
        />

        {/* Waiver consent */}
        <label className="flex items-start gap-3 mb-6 cursor-pointer">
          <input
            type="checkbox"
            checked={waiverConsent}
            onChange={(e) => setWaiverConsent(e.target.checked)}
            className="mt-1 w-5 h-5 rounded border-rp-border bg-rp-card accent-rp-red flex-shrink-0"
          />
          <span className="text-sm text-neutral-300">
            I acknowledge the risks of sim racing and agree to Racing Point&apos;s{" "}
            <span className="text-rp-red font-medium">Safety Waiver</span> and{" "}
            <span className="text-rp-red font-medium">Terms of Service</span>.
            {isMinor && " A guardian has approved my participation."}
          </span>
        </label>

        {error && (
          <p className="text-red-400 text-sm mb-4">{error}</p>
        )}

        <button
          onClick={handleSubmit}
          disabled={loading || !waiverConsent || name.trim().length < 2 || !dob}
          className="w-full bg-rp-red text-white font-semibold py-4 rounded-xl disabled:opacity-50 active:bg-rp-red-light transition-colors text-lg"
        >
          {loading ? "Registering..." : "Complete Registration"}
        </button>

        <p className="text-rp-grey text-xs text-center mt-8">
          Already registered?{" "}
          <a href="/login" className="text-rp-red font-medium">
            Sign in
          </a>
        </p>
      </div>
    </div>
  );
}
