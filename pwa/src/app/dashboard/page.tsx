"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import type { DriverProfile, CustomerStats, BillingSession, GroupSessionInfo } from "@/lib/api";
import SessionCard from "@/components/SessionCard";

export default function DashboardPage() {
  const router = useRouter();
  const [profile, setProfile] = useState<DriverProfile | null>(null);
  const [stats, setStats] = useState<CustomerStats | null>(null);
  const [recentSessions, setRecentSessions] = useState<BillingSession[]>([]);
  const [groupInvite, setGroupInvite] = useState<GroupSessionInfo | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function load() {
      try {
        const [pRes, sRes, sessRes, gRes] = await Promise.all([
          api.profile(),
          api.stats(),
          api.sessions(),
          api.groupSession(),
        ]);
        if (pRes.driver) setProfile(pRes.driver);
        if (sRes.stats) setStats(sRes.stats);
        if (sessRes.sessions) setRecentSessions(sessRes.sessions.slice(0, 3));
        if (gRes.group_session) {
          // Only show banner if user is a pending invitee
          const me = pRes.driver;
          const myMember = gRes.group_session.members.find(
            (m) => m.driver_id === me?.id
          );
          if (myMember?.status === "pending") {
            setGroupInvite(gRes.group_session);
          }
        }
      } catch {
        // network error
      } finally {
        setLoading(false);
      }
    }
    load();
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
      {/* Header */}
      <div className="mb-8">
        <p className="text-rp-grey text-sm">Welcome back</p>
        <h1 className="text-2xl font-bold text-white">
          {profile?.name || "Racer"}
        </h1>
      </div>

      {/* Wallet balance + Race CTA */}
      <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-6 flex items-center justify-between">
        <div>
          <p className="text-xs text-rp-grey">Credits</p>
          <p className="text-2xl font-bold text-white">
            {((profile?.wallet_balance_paise || 0) / 100).toFixed(0)} credits
          </p>
        </div>
        <a
          href="/book"
          className="bg-rp-red text-white font-semibold px-6 py-3 rounded-xl text-sm"
        >
          Race Now
        </a>
      </div>

      {/* Free trial CTA */}
      {profile && !profile.has_used_trial && (
        <div className="bg-emerald-900/30 border border-emerald-500/30 rounded-xl p-4 mb-6">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-emerald-400 font-semibold text-sm">Free 5-min trial available!</p>
              <p className="text-neutral-400 text-xs mt-0.5">Try sim racing for free</p>
            </div>
            <a
              href="/book?trial=true"
              className="bg-emerald-600 text-white font-semibold px-4 py-2 rounded-lg text-sm"
            >
              Start Free Trial
            </a>
          </div>
        </div>
      )}

      {/* Quick stats */}
      {stats && (
        <div className="grid grid-cols-3 gap-3 mb-8">
          <StatBox label="Sessions" value={stats.total_sessions.toString()} />
          <StatBox label="Laps" value={stats.total_laps.toString()} />
          <StatBox
            label="Drive Time"
            value={formatMinutes(stats.total_driving_seconds)}
          />
        </div>
      )}

      {/* Active session banner */}
      {recentSessions.some((s) => s.status === "active") && (
        <div className="bg-rp-red/10 border border-rp-red/30 rounded-xl p-4 mb-6">
          <div className="flex items-center gap-2 mb-1">
            <div className="w-2 h-2 bg-rp-red rounded-full animate-pulse" />
            <span className="text-rp-red font-semibold text-sm">
              Session Active
            </span>
          </div>
          {recentSessions
            .filter((s) => s.status === "active")
            .map((s) => (
              <p key={s.id} className="text-neutral-300 text-sm">
                Pod {s.pod_id.replace("pod_", "#")} —{" "}
                {formatMinutes(s.allocated_seconds - s.driving_seconds)}{" "}
                remaining
              </p>
            ))}
        </div>
      )}

      {/* Group invite banner */}
      {groupInvite && (
        <div className="bg-rp-red/10 border border-rp-red/30 rounded-xl p-4 mb-6">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-rp-red font-semibold text-sm">
                {groupInvite.host_name} invited you to race!
              </p>
              <p className="text-neutral-400 text-xs mt-0.5">
                {groupInvite.experience_name} &middot;{" "}
                {groupInvite.pricing_tier_name}
              </p>
            </div>
            <button
              onClick={() => router.push("/book/group")}
              className="bg-rp-red text-white font-semibold px-4 py-2 rounded-lg text-sm"
            >
              View
            </button>
          </div>
        </div>
      )}

      {/* Recent sessions */}
      <div className="mb-6">
        <h2 className="text-sm font-medium text-rp-grey mb-3">
          Recent Sessions
        </h2>
        {recentSessions.length === 0 ? (
          <p className="text-rp-grey text-sm">No sessions yet. Get racing!</p>
        ) : (
          <div className="space-y-3">
            {recentSessions.map((session) => (
              <SessionCard key={session.id} session={session} />
            ))}
          </div>
        )}
      </div>

      {/* Favourite car */}
      {stats?.favourite_car && (
        <div className="bg-rp-card border border-rp-border rounded-xl p-4">
          <p className="text-xs text-rp-grey mb-1">Favourite Car</p>
          <p className="text-lg font-semibold text-white">
            {stats.favourite_car}
          </p>
        </div>
      )}
    </div>
  );
}

function StatBox({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-rp-card border border-rp-border rounded-xl p-3 text-center">
      <p className="text-xl font-bold text-white">{value}</p>
      <p className="text-[10px] text-rp-grey mt-0.5">{label}</p>
    </div>
  );
}

function formatMinutes(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}
