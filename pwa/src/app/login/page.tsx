"use client";

import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import { api, setToken } from "@/lib/api";

type Step = "phone" | "otp" | "register";

export default function LoginPage() {
  const router = useRouter();
  const [step, setStep] = useState<Step>("phone");
  const [phone, setPhone] = useState("");
  const [otp, setOtp] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  // Registration fields
  const [name, setName] = useState("");
  const [nickname, setNickname] = useState("");
  const [dob, setDob] = useState("");
  const [email, setEmail] = useState("");
  const [guardianName, setGuardianName] = useState("");
  const [guardianPhone, setGuardianPhone] = useState("");
  const [waiverConsent, setWaiverConsent] = useState(false);
  const [isMinor, setIsMinor] = useState(false);

  useEffect(() => {
    if (!dob) { setIsMinor(false); return; }
    const birthDate = new Date(dob);
    const today = new Date();
    let age = today.getFullYear() - birthDate.getFullYear();
    const m = today.getMonth() - birthDate.getMonth();
    if (m < 0 || (m === 0 && today.getDate() < birthDate.getDate())) age--;
    setIsMinor(age < 18);
  }, [dob]);

  const handleSendOtp = async () => {
    if (phone.length < 10) { setError("Enter a valid phone number"); return; }
    setLoading(true);
    setError("");
    try {
      const formatted = phone.startsWith("+") ? phone : `+91${phone}`;
      const res = await api.login(formatted);
      if (res.error) { setError(res.error); } else { setStep("otp"); }
    } catch { setError("Network error. Try again."); }
    finally { setLoading(false); }
  };

  const handleVerifyOtp = async () => {
    if (otp.length !== 6) { setError("Enter the 6-digit code"); return; }
    setLoading(true);
    setError("");
    try {
      const formatted = phone.startsWith("+") ? phone : `+91${phone}`;
      const res = await api.verifyOtp(formatted, otp) as {
        error?: string; token?: string; registration_completed?: boolean;
      };
      if (res.error) {
        setError(res.error);
      } else if (res.token) {
        setToken(res.token);
        if (res.registration_completed === false) {
          setStep("register");
        } else {
          router.replace("/dashboard");
        }
      }
    } catch { setError("Network error. Try again."); }
    finally { setLoading(false); }
  };

  const handleRegister = async () => {
    if (name.trim().length < 2) { setError("Name must be at least 2 characters"); return; }
    if (!dob) { setError("Date of birth is required"); return; }
    if (!waiverConsent) { setError("You must accept the safety waiver"); return; }
    if (isMinor && !guardianName.trim()) { setError("Guardian name is required for under 18"); return; }
    setLoading(true);
    setError("");
    try {
      const res = await api.register({
        name: name.trim(), dob,
        nickname: nickname.trim() || undefined,
        email: email.trim() || undefined,
        waiver_consent: waiverConsent,
        guardian_name: isMinor ? guardianName.trim() : undefined,
        guardian_phone: isMinor ? guardianPhone.trim() || undefined : undefined,
      });
      if (res.error) { setError(res.error); } else { router.replace("/dashboard"); }
    } catch { setError("Network error. Try again."); }
    finally { setLoading(false); }
  };

  return (
    <div className="flex flex-col items-center justify-center min-h-screen px-6 py-12">
      {/* Logo */}
      <div className={`${step === "register" ? "mb-12" : "mb-20"} text-center`}>
        <h1 className={`${step === "register" ? "text-4xl" : "text-5xl"} font-black tracking-tight`}>
          <span className="text-rp-red">Racing</span>
          <span className="text-white">Point</span>
        </h1>
        <p className="text-rp-grey text-sm mt-4 tracking-widest uppercase">
          May the Fastest Win
        </p>
      </div>

      <div className="w-full max-w-sm">
        {step === "phone" && (
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

            {error && <p className="text-red-400 text-sm mb-4">{error}</p>}

            <button
              onClick={handleSendOtp}
              disabled={loading || phone.length < 10}
              className="w-full bg-rp-red text-white font-semibold py-4 rounded-xl disabled:opacity-50 active:bg-rp-red-light transition-colors text-lg"
            >
              {loading ? "Sending..." : "Send OTP via WhatsApp"}
            </button>
          </>
        )}

        {step === "otp" && (
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

            {error && <p className="text-red-400 text-sm mb-4">{error}</p>}

            <button
              onClick={handleVerifyOtp}
              disabled={loading || otp.length !== 6}
              className="w-full bg-rp-red text-white font-semibold py-4 rounded-xl disabled:opacity-50 active:bg-rp-red-light transition-colors text-lg"
            >
              {loading ? "Verifying..." : "Verify & Sign In"}
            </button>
          </>
        )}

        {step === "register" && (
          <>
            <h2 className="text-2xl font-bold mb-2">Complete Your Profile</h2>
            <p className="text-neutral-400 text-sm mb-8">
              One last step before you hit the track
            </p>

            <label className="block text-sm font-medium text-neutral-400 mb-2">Full Name *</label>
            <input
              type="text" value={name} onChange={(e) => setName(e.target.value)}
              placeholder="Your name"
              className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3.5 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-5"
              autoFocus
            />

            <label className="block text-sm font-medium text-neutral-400 mb-2">
              Nickname <span className="text-zinc-600">(shown on leaderboard)</span>
            </label>
            <input
              type="text" value={nickname} onChange={(e) => setNickname(e.target.value)}
              placeholder="Your gamertag"
              className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3.5 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-5"
            />

            <label className="block text-sm font-medium text-neutral-400 mb-2">Date of Birth *</label>
            <input
              type="date" value={dob} onChange={(e) => setDob(e.target.value)}
              max={new Date().toISOString().split("T")[0]}
              className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3.5 text-white focus:outline-none focus:border-rp-red transition-colors mb-5 [color-scheme:dark]"
            />

            {isMinor && (
              <div className="bg-amber-500/10 border border-amber-500/30 rounded-xl p-4 mb-5">
                <p className="text-amber-400 text-sm font-medium mb-3">Under 18 — Guardian details required</p>
                <label className="block text-sm font-medium text-neutral-400 mb-2">Guardian Name *</label>
                <input
                  type="text" value={guardianName} onChange={(e) => setGuardianName(e.target.value)}
                  placeholder="Parent or guardian name"
                  className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-3"
                />
                <label className="block text-sm font-medium text-neutral-400 mb-2">Guardian Phone</label>
                <input
                  type="tel" value={guardianPhone}
                  onChange={(e) => setGuardianPhone(e.target.value.replace(/\D/g, "").slice(0, 10))}
                  placeholder="Phone number"
                  className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors"
                  inputMode="numeric"
                />
              </div>
            )}

            <label className="block text-sm font-medium text-neutral-400 mb-2">
              Email <span className="text-zinc-600">(optional)</span>
            </label>
            <input
              type="email" value={email} onChange={(e) => setEmail(e.target.value)}
              placeholder="you@example.com"
              className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3.5 text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-6"
            />

            <label className="flex items-start gap-3 mb-6 cursor-pointer">
              <input
                type="checkbox" checked={waiverConsent} onChange={(e) => setWaiverConsent(e.target.checked)}
                className="mt-1 w-5 h-5 rounded border-rp-border bg-rp-card accent-rp-red flex-shrink-0"
              />
              <span className="text-sm text-neutral-300">
                I acknowledge the risks of sim racing and agree to Racing Point&apos;s{" "}
                <span className="text-rp-red font-medium">Safety Waiver</span> and{" "}
                <span className="text-rp-red font-medium">Terms of Service</span>.
                {isMinor && " A guardian has approved my participation."}
              </span>
            </label>

            {error && <p className="text-red-400 text-sm mb-4">{error}</p>}

            <button
              onClick={handleRegister}
              disabled={loading || !waiverConsent || name.trim().length < 2 || !dob}
              className="w-full bg-rp-red text-white font-semibold py-4 rounded-xl disabled:opacity-50 active:bg-rp-red-light transition-colors text-lg"
            >
              {loading ? "Registering..." : "Complete Registration"}
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
