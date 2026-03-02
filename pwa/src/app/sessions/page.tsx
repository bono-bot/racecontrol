"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { BillingSession } from "@/lib/api";
import BottomNav from "@/components/BottomNav";
import SessionCard from "@/components/SessionCard";

export default function SessionsPage() {
  const router = useRouter();
  const [sessions, setSessions] = useState<BillingSession[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    api.sessions().then((res) => {
      if (res.sessions) setSessions(res.sessions);
      setLoading(false);
    });
  }, [router]);

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <h1 className="text-2xl font-bold text-zinc-100 mb-6">Sessions</h1>

        {loading ? (
          <div className="flex justify-center py-12">
            <div className="w-8 h-8 border-2 border-rp-orange border-t-transparent rounded-full animate-spin" />
          </div>
        ) : sessions.length === 0 ? (
          <div className="text-center py-12">
            <p className="text-zinc-500">No sessions yet</p>
            <p className="text-zinc-600 text-sm mt-1">
              Visit RacingPoint to start your first session
            </p>
          </div>
        ) : (
          <div className="space-y-3">
            {sessions.map((session) => (
              <SessionCard key={session.id} session={session} />
            ))}
          </div>
        )}
      </div>
      <BottomNav />
    </div>
  );
}
