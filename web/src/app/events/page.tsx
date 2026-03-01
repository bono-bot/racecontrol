"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import StatusBadge from "@/components/StatusBadge";
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
          <h1 className="text-2xl font-bold text-zinc-100">Events</h1>
          <p className="text-sm text-zinc-500">Tournaments, championships, and competitions</p>
        </div>
      </div>

      {loading ? (
        <div className="text-center py-12 text-zinc-500 text-sm">Loading events...</div>
      ) : events.length === 0 ? (
        <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-8 text-center">
          <p className="text-zinc-400 mb-2">No events scheduled</p>
          <p className="text-zinc-500 text-sm">
            Create race events, hotlap competitions, and tournaments from the API.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {events.map((event) => (
            <div
              key={event.id}
              className="flex items-center justify-between bg-zinc-900 border border-zinc-800 rounded-lg px-4 py-3"
            >
              <div className="flex items-center gap-4">
                <span className="text-zinc-200 font-medium">{event.name}</span>
                <span className="text-xs text-zinc-500 capitalize">{event.type}</span>
              </div>
              <StatusBadge status={event.status} />
            </div>
          ))}
        </div>
      )}
    </DashboardLayout>
  );
}
