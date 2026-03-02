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

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    api.profile().then((res) => {
      if (res.driver) setProfile(res.driver);
      setLoading(false);
    });
  }, [router]);

  const handleLogout = () => {
    clearToken();
    router.replace("/login");
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
                  {profile.name.charAt(0).toUpperCase()}
                </span>
              </div>
              <div>
                <h2 className="text-xl font-bold text-white">
                  {profile.name}
                </h2>
                <p className="text-sm text-rp-grey">
                  {profile.phone || profile.email || "No contact info"}
                </p>
              </div>
            </div>

            {/* Info cards */}
            <div className="space-y-3 mb-8">
              <InfoRow label="Name" value={profile.name} />
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
