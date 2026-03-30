"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { Skeleton, EmptyState } from "@/components/Skeleton";
import StatusBadge from "@/components/StatusBadge";
import { Calendar } from "lucide-react";
import type { RaceEvent } from "@/lib/api";
import { api } from "@/lib/api";

export default function EventsPage() {
  const [events, setEvents] = useState<RaceEvent[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.listEvents().then((res) => {
      setEvents(res.events || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Events</h1>
          <p className="text-sm text-rp-grey">Tournaments, championships, and competitions</p>
        </div>
      </div>

      {loading ? (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} className="h-10 rounded-lg" />
          ))}
        </div>
      ) : events.length === 0 ? (
        <EmptyState
          icon={<Calendar className="w-10 h-10" />}
          headline="No events scheduled"
          hint="Create race events, hotlap competitions, and tournaments from the API."
        />
      ) : (
        <div className="space-y-2">
          {events.map((event) => (
            <div
              key={event.id}
              className="flex items-center justify-between bg-rp-card border border-rp-border rounded-lg px-4 py-3"
            >
              <div className="flex items-center gap-4">
                <span className="text-neutral-200 font-medium">{event.name}</span>
                <span className="text-xs text-rp-grey capitalize">{event.type}</span>
              </div>
              <StatusBadge status={event.status} />
            </div>
          ))}
        </div>
      )}
    </DashboardLayout>
  );
}
