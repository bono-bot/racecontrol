"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { Skeleton, EmptyState } from "@/components/Skeleton";
import StatusBadge from "@/components/StatusBadge";
import { BookOpen } from "lucide-react";
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
          <h1 className="text-2xl font-bold text-white">Bookings</h1>
          <p className="text-sm text-rp-grey">Pod reservations and time slots</p>
        </div>
      </div>

      {loading ? (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} className="h-10 rounded-lg" />
          ))}
        </div>
      ) : bookings.length === 0 ? (
        <EmptyState
          icon={<BookOpen className="w-10 h-10" />}
          headline="No bookings yet"
          hint="Bookings can be made via the kiosk or API."
        />
      ) : (
        <div className="space-y-2">
          {bookings.map((booking) => (
            <div
              key={booking.id}
              className="flex items-center justify-between bg-rp-card border border-rp-border rounded-lg px-4 py-3"
            >
              <div className="flex items-center gap-4">
                <span className="text-neutral-300 text-sm">
                  {new Date(booking.start_time).toLocaleString()} &mdash;{" "}
                  {new Date(booking.end_time).toLocaleTimeString()}
                </span>
                <span className="text-xs text-rp-grey">
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
