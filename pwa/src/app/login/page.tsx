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
      const res = await api.verifyOtp(formatted, otp) as {
        error?: string;
        token?: string;
        registration_completed?: boolean;
      };
      if (res.error) {
        setError(res.error);
      } else if (res.token) {
        setToken(res.token);
        if (res.registration_completed === false) {
          router.replace("/register");
        } else {
          router.replace("/dashboard");
        }
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
      <div className="mb-20 text-center">
        <h1 className="text-5xl font-black tracking-tight">
          <span className="text-rp-red">Racing</span>
          <span className="text-white">Point</span>
        </h1>
        <p className="text-rp-grey text-sm mt-4 tracking-widest uppercase">
          May the Fastest Win
        </p>
      </div>

      <div className="w-full max-w-sm">
        {step === "phone" ? (
          <>
            <h2 className="text-2xl font-bold mb-3">Sign In</h2>
            <p className="text-neutral-400 text-sm mb-10">
              Enter your phone number to continue
            </p>

            <label className="block text-sm font-medium text-neutral-400 mb-3">
              Phone number
            </label>
            <div className="flex items-center gap-3 mb-8">
              <span className="bg-rp-card border border-rp-border rounded-xl px-4 py-4 text-rp-grey text-lg font-medium">
                +91
              </span>
              <input
                type="tel"
                value={phone}
                onChange={(e) => setPhone(e.target.value.replace(/\D/g, "").slice(0, 10))}
                placeholder="98765 43210"
                className="flex-1 bg-rp-card border border-rp-border rounded-xl px-4 py-4 text-lg text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors"
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
              className="w-full bg-rp-red text-white font-semibold py-4 rounded-xl disabled:opacity-50 active:bg-rp-red-light transition-colors text-lg"
            >
              {loading ? "Sending..." : "Send OTP via WhatsApp"}
            </button>

            <p className="text-rp-grey text-sm text-center mt-8">
              New here? <a href="/register" className="text-rp-red font-medium">Register first</a>
            </p>
          </>
        ) : (
          <>
            <h2 className="text-2xl font-bold mb-3">Verify OTP</h2>
            <p className="text-neutral-400 text-sm mb-3">
              Enter the 6-digit code sent to your WhatsApp
            </p>
            <p className="text-neutral-500 text-sm mb-10">
              +91 {phone}{" "}
              <button
                onClick={() => { setStep("phone"); setOtp(""); setError(""); }}
                className="text-rp-red font-medium"
              >
                Change
              </button>
            </p>

            <label className="block text-sm font-medium text-neutral-400 mb-3">
              Verification code
            </label>
            <input
              type="tel"
              value={otp}
              onChange={(e) => setOtp(e.target.value.replace(/\D/g, "").slice(0, 6))}
              placeholder="6-digit code"
              className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-4 text-lg text-center tracking-[0.5em] text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-8"
              autoFocus
              inputMode="numeric"
            />

            {error && (
              <p className="text-red-400 text-sm mb-4">{error}</p>
            )}

            <button
              onClick={handleVerifyOtp}
              disabled={loading || otp.length !== 6}
              className="w-full bg-rp-red text-white font-semibold py-4 rounded-xl disabled:opacity-50 active:bg-rp-red-light transition-colors text-lg"
            >
              {loading ? "Verifying..." : "Verify & Sign In"}
            </button>
          </>
        )}

        <p className="text-rp-grey text-xs text-center mt-12">
          By signing in, you agree to our Terms of Service
        </p>
      </div>
    </div>
  );
}
