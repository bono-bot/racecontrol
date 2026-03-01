"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import DashboardLayout from "@/components/DashboardLayout";
import LiveLapFeed from "@/components/LiveLapFeed";
import type { Lap } from "@/lib/api";
import { api } from "@/lib/api";

export default function ResultsPage() {
  const params = useParams();
  const sessionId = params.id as string;
  const [laps, setLaps] = useState<Lap[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.sessionLaps(sessionId).then((res) => {
      setLaps(res.laps || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, [sessionId]);

  return (
    <DashboardLayout>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-zinc-100">Session Results</h1>
        <p className="text-sm text-zinc-500 font-mono">{sessionId}</p>
      </div>

      {loading ? (
        <div className="text-center py-12 text-zinc-500 text-sm">Loading results...</div>
      ) : (
        <LiveLapFeed laps={laps} />
      )}
    </DashboardLayout>
  );
}
