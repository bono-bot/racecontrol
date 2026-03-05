"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { TournamentInfo } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

const statusColors: Record<string, string> = {
  upcoming: "bg-blue-500/20 text-blue-400 border-blue-500/30",
  registration: "bg-emerald-500/20 text-emerald-400 border-emerald-500/30",
  in_progress: "bg-yellow-500/20 text-yellow-400 border-yellow-500/30",
  completed: "bg-neutral-500/20 text-neutral-300 border-neutral-500/30",
};

export default function TournamentsPage() {
  const router = useRouter();
  const [tournaments, setTournaments] = useState<TournamentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [registering, setRegistering] = useState<string | null>(null);

  useEffect(() => {
    if (!isLoggedIn()) { router.replace("/login"); return; }
    api.tournaments().then((res) => {
      setTournaments(res.tournaments || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, [router]);

  const handleRegister = async (id: string) => {
    setRegistering(id);
    const res = await api.registerTournament(id);
    if (res.ok) {
      setTournaments((prev) =>
        prev.map((t) => (t.id === id ? { ...t, is_registered: true } : t))
      );
    } else {
      alert(res.error || "Registration failed");
    }
    setRegistering(null);
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <h1 className="text-2xl font-bold text-white mb-1">Tournaments</h1>
        <p className="text-rp-grey text-sm mb-6">Compete. Win. Prove yourself.</p>

        {tournaments.length === 0 && (
          <div className="bg-rp-card border border-rp-border rounded-xl p-8 text-center">
            <p className="text-rp-grey">No tournaments scheduled yet</p>
            <p className="text-rp-grey text-xs mt-1">Check back soon!</p>
          </div>
        )}

        <div className="space-y-4">
          {tournaments.map((t) => (
            <div key={t.id} className="bg-rp-card border border-rp-border rounded-xl p-4">
              <div className="flex items-start justify-between mb-2">
                <h2 className="text-white font-bold">{t.name}</h2>
                <span className={`text-[10px] font-semibold uppercase tracking-wider px-2 py-0.5 rounded-full border ${statusColors[t.status] || statusColors.upcoming}`}>
                  {t.status.replace("_", " ")}
                </span>
              </div>

              {t.description && <p className="text-rp-grey text-sm mb-3">{t.description}</p>}

              <div className="grid grid-cols-2 gap-2 text-xs mb-3">
                <div>
                  <span className="text-rp-grey">Track: </span>
                  <span className="text-white">{t.track}</span>
                </div>
                <div>
                  <span className="text-rp-grey">Car: </span>
                  <span className="text-white">{t.car}</span>
                </div>
                <div>
                  <span className="text-rp-grey">Format: </span>
                  <span className="text-white capitalize">{t.format.replace("_", " ")}</span>
                </div>
                <div>
                  <span className="text-rp-grey">Slots: </span>
                  <span className="text-white">{t.max_participants}</span>
                </div>
                <div>
                  <span className="text-rp-grey">Entry: </span>
                  <span className="text-white">{t.entry_fee_display}</span>
                </div>
                <div>
                  <span className="text-rp-grey">Prize: </span>
                  <span className="text-rp-red font-bold">{t.prize_pool_display}</span>
                </div>
              </div>

              {t.event_date && (
                <p className="text-rp-grey text-xs mb-3">
                  Date: {new Date(t.event_date).toLocaleDateString("en-IN", { weekday: "short", day: "numeric", month: "short" })}
                </p>
              )}

              {t.is_registered ? (
                <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-lg py-2 text-center">
                  <span className="text-emerald-400 text-sm font-medium">Registered</span>
                </div>
              ) : (t.status === "registration" || t.status === "upcoming") ? (
                <button
                  onClick={() => handleRegister(t.id)}
                  disabled={registering === t.id}
                  className="w-full bg-rp-red hover:bg-rp-red/90 text-white font-semibold py-2.5 rounded-lg transition-colors disabled:opacity-50"
                >
                  {registering === t.id ? "Registering..." : "Register Now"}
                </button>
              ) : null}
            </div>
          ))}
        </div>
      </div>
      <BottomNav />
    </div>
  );
}
