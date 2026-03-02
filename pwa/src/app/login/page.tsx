"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, setToken } from "@/lib/api";

type Step = "phone" | "otp";

export default function LoginPage() {
  const router = useRouter();
  const [step, setStep] = useState<Step>("phone");
  const [phone, setPhone] = useState("");
  const [otp, setOtp] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const handleSendOtp = async () => {
    if (phone.length < 10) {
      setError("Enter a valid phone number");
      return;
    }

    setLoading(true);
    setError("");

    try {
      const formatted = phone.startsWith("+") ? phone : `+91${phone}`;
      const res = await api.login(formatted);
      if (res.error) {
        setError(res.error);
      } else {
        setStep("otp");
      }
    } catch {
      setError("Network error. Try again.");
    } finally {
      setLoading(false);
    }
  };

  const handleVerifyOtp = async () => {
    if (otp.length !== 6) {
      setError("Enter the 6-digit code");
      return;
    }

    setLoading(true);
    setError("");

    try {
      const formatted = phone.startsWith("+") ? phone : `+91${phone}`;
      const res = await api.verifyOtp(formatted, otp);
      if (res.error) {
        setError(res.error);
      } else if (res.token) {
        setToken(res.token);
        router.replace("/dashboard");
      }
    } catch {
      setError("Network error. Try again.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col items-center justify-center min-h-screen px-6">
      {/* Logo */}
      <div className="mb-12 text-center">
        <h1 className="text-5xl font-black tracking-tight">
          <span className="text-rp-orange">Racing</span>
          <span className="text-zinc-100">Point</span>
        </h1>
        <p className="text-zinc-500 text-sm mt-2">Sim Racing Experience</p>
      </div>

      <div className="w-full max-w-sm">
        {step === "phone" ? (
          <>
            <label className="block text-sm font-medium text-zinc-400 mb-2">
              Phone number
            </label>
            <div className="flex items-center gap-2 mb-4">
              <span className="text-zinc-500 text-lg font-medium">+91</span>
              <input
                type="tel"
                value={phone}
                onChange={(e) => setPhone(e.target.value.replace(/\D/g, "").slice(0, 10))}
                placeholder="Enter your number"
                className="flex-1 bg-rp-card border border-rp-border rounded-xl px-4 py-3.5 text-lg text-zinc-100 placeholder-zinc-600 focus:outline-none focus:border-rp-orange transition-colors"
                autoFocus
                inputMode="numeric"
              />
            </div>

            {error && (
              <p className="text-red-400 text-sm mb-4">{error}</p>
            )}

            <button
              onClick={handleSendOtp}
              disabled={loading || phone.length < 10}
              className="w-full bg-rp-orange text-white font-semibold py-3.5 rounded-xl disabled:opacity-50 active:bg-rp-orange-light transition-colors"
            >
              {loading ? "Sending..." : "Send verification code"}
            </button>
          </>
        ) : (
          <>
            <p className="text-sm text-zinc-400 mb-1">
              Code sent to +91 {phone}
            </p>
            <button
              onClick={() => { setStep("phone"); setOtp(""); setError(""); }}
              className="text-rp-orange text-sm mb-4 inline-block"
            >
              Change number
            </button>

            <label className="block text-sm font-medium text-zinc-400 mb-2">
              Verification code
            </label>
            <input
              type="tel"
              value={otp}
              onChange={(e) => setOtp(e.target.value.replace(/\D/g, "").slice(0, 6))}
              placeholder="6-digit code"
              className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3.5 text-lg text-center tracking-[0.5em] text-zinc-100 placeholder-zinc-600 focus:outline-none focus:border-rp-orange transition-colors mb-4"
              autoFocus
              inputMode="numeric"
            />

            {error && (
              <p className="text-red-400 text-sm mb-4">{error}</p>
            )}

            <button
              onClick={handleVerifyOtp}
              disabled={loading || otp.length !== 6}
              className="w-full bg-rp-orange text-white font-semibold py-3.5 rounded-xl disabled:opacity-50 active:bg-rp-orange-light transition-colors"
            >
              {loading ? "Verifying..." : "Verify & Sign In"}
            </button>
          </>
        )}

        <p className="text-zinc-600 text-xs text-center mt-8">
          By signing in, you agree to our Terms of Service
        </p>
      </div>
    </div>
  );
}
