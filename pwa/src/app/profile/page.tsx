"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn, clearToken } from "@/lib/api";
import type { DriverProfile } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

export default function ProfilePage() {
  const router = useRouter();
  const [profile, setProfile] = useState<DriverProfile | null>(null);
  const [loading, setLoading] = useState(true);

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
