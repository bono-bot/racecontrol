"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn, clearToken } from "@/lib/api";
import type { DriverProfile, Badge } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

export default function ProfilePage() {
  const router = useRouter();
  const [profile, setProfile] = useState<DriverProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [badges, setBadges] = useState<Badge[] | null>(null);
  const [passportSummary, setPassportSummary] = useState<{
    unique_tracks: number;
    unique_cars: number;
  } | null>(null);

  // Nickname editing
  const [editingNickname, setEditingNickname] = useState(false);
  const [nicknameInput, setNicknameInput] = useState("");
  const [showNickname, setShowNickname] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    api.profile().then((res) => {
      if (res.driver) {
        setProfile(res.driver);
        setNicknameInput(res.driver.nickname || "");
        setShowNickname(res.driver.show_nickname_on_leaderboard || false);
      }
      setLoading(false);
    }).catch(() => setLoading(false));

    api.badges().then((res) => {
      if (res.badges) {
        setBadges([...res.badges.earned, ...res.badges.available]);
      }
    });

    api.passport().then((res) => {
      if (res.passport?.summary) {
        setPassportSummary({
          unique_tracks: res.passport.summary.unique_tracks,
          unique_cars: res.passport.summary.unique_cars,
        });
      }
    });
  }, [router]);

  const handleLogout = () => {
    clearToken();
    router.replace("/login");
  };

  const handleSaveNickname = async () => {
    setSaving(true);
    await api.updateProfile({ nickname: nicknameInput.trim() || undefined });
    if (profile) setProfile({ ...profile, nickname: nicknameInput.trim() || null });
    setEditingNickname(false);
    setSaving(false);
  };

  const handleToggleLeaderboard = async () => {
    const newVal = !showNickname;
    setShowNickname(newVal);
    await api.updateProfile({ show_nickname_on_leaderboard: newVal });
    if (profile) setProfile({ ...profile, show_nickname_on_leaderboard: newVal });
  };

  if (loading) {
    return (
      <div className="min-h-screen pb-20 flex items-center justify-center">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <h1 className="text-2xl font-bold text-white mb-8">Profile</h1>

        {profile && (
          <>
            {/* Avatar + name */}
            <div className="flex items-center gap-4 mb-8">
              <div className="w-16 h-16 rounded-full bg-rp-red/20 flex items-center justify-center">
                <span className="text-2xl font-bold text-rp-red">
                  {(profile.nickname || profile.name).charAt(0).toUpperCase()}
                </span>
              </div>
              <div>
                <h2 className="text-xl font-bold text-white">
                  {profile.nickname || profile.name}
                </h2>
                {profile.nickname && (
                  <p className="text-xs text-rp-grey">{profile.name}</p>
                )}
                {profile.customer_id && (
                  <p className="text-xs font-mono text-rp-red">{profile.customer_id}</p>
                )}
                <p className="text-sm text-rp-grey">
                  {profile.phone || profile.email || "No contact info"}
                </p>
              </div>
            </div>

            {/* Wallet */}
            <button
              onClick={() => router.push("/wallet/history")}
              className="w-full bg-rp-card border border-rp-border rounded-xl p-4 mb-6 flex items-center justify-between active:bg-rp-card/80 transition-colors text-left"
            >
              <div>
                <p className="text-xs text-rp-grey">Credits</p>
                <p className="text-2xl font-bold text-white">
                  {((profile.wallet_balance_paise || 0) / 100).toFixed(0)} credits
                </p>
              </div>
              <span className="text-xs text-rp-red font-medium">History &rarr;</span>
            </button>

            {/* Badge Showcase */}
            {badges && (
              <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-6">
                <div className="flex items-center justify-between mb-3">
                  <h3 className="text-sm font-bold text-white">Badges</h3>
                  <span className="text-xs text-rp-grey">
                    {badges.filter((b) => b.earned).length} / {badges.length}
                  </span>
                </div>
                <div className="grid grid-cols-4 gap-3">
                  {badges.map((badge) => (
                    <div
                      key={badge.id}
                      className={`flex flex-col items-center ${badge.earned ? "" : "opacity-30"}`}
                    >
                      <div
                        className={`w-12 h-12 rounded-full flex items-center justify-center mb-1 ${
                          badge.earned
                            ? "bg-rp-red/20"
                            : "bg-rp-card border border-rp-border"
                        }`}
                      >
                        <BadgeIcon icon={badge.icon} earned={badge.earned} />
                      </div>
                      <span className="text-xs text-center text-rp-grey">
                        {badge.name}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Info cards */}
            <div className="space-y-3 mb-6">
              <InfoRow label="Name" value={profile.name} />

              {/* Nickname row */}
              <div className="bg-rp-card border border-rp-border rounded-xl p-4">
                <div className="flex justify-between items-center">
                  <span className="text-sm text-rp-grey">Nickname</span>
                  {!editingNickname ? (
                    <button
                      onClick={() => setEditingNickname(true)}
                      className="text-sm text-rp-red font-medium"
                    >
                      {profile.nickname || "Set nickname"}
                    </button>
                  ) : (
                    <div className="flex items-center gap-2">
                      <input
                        type="text"
                        value={nicknameInput}
                        onChange={(e) => setNicknameInput(e.target.value)}
                        placeholder="Gamertag"
                        className="bg-rp-bg border border-rp-border rounded-lg px-3 py-1.5 text-sm text-white w-32 focus:outline-none focus:border-rp-red"
                        autoFocus
                      />
                      <button
                        onClick={handleSaveNickname}
                        disabled={saving}
                        className="text-xs text-rp-red font-medium"
                      >
                        {saving ? "..." : "Save"}
                      </button>
                      <button
                        onClick={() => { setEditingNickname(false); setNicknameInput(profile.nickname || ""); }}
                        className="text-xs text-rp-grey"
                      >
                        Cancel
                      </button>
                    </div>
                  )}
                </div>
              </div>

              {/* Leaderboard display toggle */}
              {profile.nickname && (
                <div className="bg-rp-card border border-rp-border rounded-xl p-4 flex justify-between items-center">
                  <span className="text-sm text-rp-grey">Show nickname on leaderboard</span>
                  <button
                    onClick={handleToggleLeaderboard}
                    className={`w-11 h-6 rounded-full transition-colors relative ${showNickname ? "bg-rp-red" : "bg-zinc-700"}`}
                  >
                    <span className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${showNickname ? "left-[22px]" : "left-0.5"}`} />
                  </button>
                </div>
              )}

              <InfoRow label="Phone" value={profile.phone || "Not set"} />
              <InfoRow label="Email" value={profile.email || "Not set"} />
              <InfoRow label="Total Laps" value={profile.total_laps.toString()} />
              <InfoRow
                label="Total Time"
                value={formatHours(profile.total_time_ms / 1000)}
              />
            </div>

            {/* Links */}
            <div className="space-y-2 mb-8">
              {/* Driving Passport */}
              <button
                onClick={() => router.push("/passport")}
                className="w-full bg-rp-card border border-rp-border rounded-xl p-4 text-left active:bg-rp-card/80 transition-colors"
              >
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-xs text-rp-grey">Driving Passport</p>
                    <p className="text-sm font-bold text-white">
                      {passportSummary
                        ? `${passportSummary.unique_tracks} circuits \u00B7 ${passportSummary.unique_cars} cars driven`
                        : "Start your journey"}
                    </p>
                  </div>
                  <span className="text-xs text-rp-red font-bold">
                    View &rarr;
                  </span>
                </div>
              </button>
              <button
                onClick={() => router.push("/friends")}
                className="w-full bg-rp-card border border-rp-border rounded-xl p-4 text-left text-sm text-neutral-300 active:bg-rp-card transition-colors flex items-center justify-between"
              >
                <span>Friends</span>
                <span className="text-rp-grey text-xs">Manage</span>
              </button>
              <button
                onClick={() => router.push("/leaderboard")}
                className="w-full bg-rp-card border border-rp-border rounded-xl p-4 text-left text-sm text-neutral-300 active:bg-rp-card transition-colors"
              >
                View Leaderboard
              </button>
            </div>

            {/* Logout */}
            <button
              onClick={handleLogout}
              className="w-full bg-red-500/10 border border-red-500/30 text-red-400 font-medium py-3 rounded-xl active:bg-red-500/20 transition-colors"
            >
              Sign Out
            </button>
          </>
        )}
      </div>
      <BottomNav />
    </div>
  );
}

function BadgeIcon({ icon, earned }: { icon: string; earned: boolean }) {
  const color = earned ? "text-rp-red" : "text-rp-grey";
  const svgProps = {
    viewBox: "0 0 24 24",
    fill: "none" as const,
    stroke: "currentColor",
    strokeWidth: 2,
    className: `w-5 h-5 ${color}`,
  };

  switch (icon) {
    case "flag":
      return (
        <svg {...svgProps}>
          <path d="M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z" />
          <line x1="4" y1="22" x2="4" y2="15" />
        </svg>
      );
    case "map":
      return (
        <svg {...svgProps}>
          <polygon points="1 6 1 22 8 18 16 22 23 18 23 2 16 6 8 2 1 6" />
          <line x1="8" y1="2" x2="8" y2="18" />
          <line x1="16" y1="6" x2="16" y2="22" />
        </svg>
      );
    case "trophy":
      return (
        <svg {...svgProps}>
          <polyline points="14 9 9 9 9 2 15 2" />
          <path d="M6 2H3v7a6 6 0 0 0 6 6" />
          <path d="M18 2h3v7a6 6 0 0 0-6 6" />
          <line x1="12" y1="15" x2="12" y2="22" />
          <line x1="8" y1="22" x2="16" y2="22" />
        </svg>
      );
    case "fire":
      return (
        <svg {...svgProps}>
          <path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 3z" />
        </svg>
      );
    case "car":
      return (
        <svg {...svgProps}>
          <rect x="1" y="3" width="15" height="13" />
          <polygon points="16 8 20 8 23 11 23 16 16 16 16 8" />
          <circle cx="5.5" cy="18.5" r="2.5" />
          <circle cx="18.5" cy="18.5" r="2.5" />
        </svg>
      );
    default:
      return (
        <svg {...svgProps}>
          <circle cx="12" cy="12" r="10" />
        </svg>
      );
  }
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-rp-card border border-rp-border rounded-xl p-4 flex justify-between items-center">
      <span className="text-sm text-rp-grey">{label}</span>
      <span className="text-sm font-medium text-neutral-200">{value}</span>
    </div>
  );
}

function formatHours(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}
