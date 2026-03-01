"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import StatusBadge from "@/components/StatusBadge";
import type { Booking } from "@/lib/api";
import { api } from "@/lib/api";

export default function BookingsPage() {
  const [bookings, setBookings] = useState<Booking[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.listBookings().then((res) => {
      setBookings(res.bookings || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-zinc-100">Bookings</h1>
          <p className="text-sm text-zinc-500">Pod reservations and time slots</p>
        </div>
      </div>

      {loading ? (
        <div className="text-center py-12 text-zinc-500 text-sm">Loading bookings...</div>
      ) : bookings.length === 0 ? (
        <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-8 text-center">
          <p className="text-zinc-400 mb-2">No bookings</p>
          <p className="text-zinc-500 text-sm">
            Bookings can be made via the kiosk or API.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {bookings.map((booking) => (
            <div
              key={booking.id}
              className="flex items-center justify-between bg-zinc-900 border border-zinc-800 rounded-lg px-4 py-3"
            >
              <div className="flex items-center gap-4">
                <span className="text-zinc-300 text-sm">
                  {new Date(booking.start_time).toLocaleString()} &mdash;{" "}
                  {new Date(booking.end_time).toLocaleTimeString()}
                </span>
                <span className="text-xs text-zinc-500">
                  Driver: {booking.driver_id?.slice(0, 8)}
                </span>
              </div>
              <StatusBadge status={booking.status} />
            </div>
          ))}
        </div>
      )}
    </DashboardLayout>
  );
}
