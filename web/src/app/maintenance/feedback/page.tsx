"use client";

import { useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";

interface FeedbackEntry {
  id: string;
  pod_id: number;
  date: string;
  rating: number;
  comment: string;
}

const MOCK_FEEDBACK: FeedbackEntry[] = [
  { id: "fb-1", pod_id: 3, date: "2026-03-30", rating: 5, comment: "Wheel calibration spot on after recalibration." },
  { id: "fb-2", pod_id: 7, date: "2026-03-30", rating: 4, comment: "Monitor alignment fixed, slight color shift remains." },
  { id: "fb-3", pod_id: 1, date: "2026-03-29", rating: 5, comment: "Pedal sensor replaced — feels better than new." },
  { id: "fb-4", pod_id: 5, date: "2026-03-29", rating: 3, comment: "FFB still feels weaker on left turns after motor service." },
  { id: "fb-5", pod_id: 2, date: "2026-03-28", rating: 4, comment: "USB hub swap resolved disconnection issues." },
  { id: "fb-6", pod_id: 6, date: "2026-03-28", rating: 5, comment: "Full pod reset — everything running perfectly." },
];

function StarRating({ rating }: { rating: number }) {
  return (
    <div className="flex gap-0.5">
      {[1, 2, 3, 4, 5].map((star) => (
        <svg
          key={star}
          className={`w-4 h-4 ${star <= rating ? "text-yellow-400" : "text-neutral-600"}`}
          fill="currentColor"
          viewBox="0 0 20 20"
        >
          <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z" />
        </svg>
      ))}
    </div>
  );
}

export default function MaintenanceFeedbackPage() {
  const [feedback] = useState<FeedbackEntry[]>(MOCK_FEEDBACK);

  const avgRating = feedback.length > 0
    ? (feedback.reduce((sum, f) => sum + f.rating, 0) / feedback.length).toFixed(1)
    : "--";

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Maintenance Feedback</h1>
          <p className="text-sm text-rp-grey">Post-maintenance quality ratings from pod users</p>
        </div>
        <div className="text-right">
          <p className="text-2xl font-bold text-yellow-400">{avgRating}</p>
          <p className="text-xs text-rp-grey">Avg rating</p>
        </div>
      </div>

      <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-rp-border text-left">
                <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Pod</th>
                <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Date</th>
                <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Rating</th>
                <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Comment</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-rp-border">
              {feedback.map((entry) => (
                <tr key={entry.id} className="hover:bg-white/5 transition-colors">
                  <td className="px-4 py-3 text-xs font-mono text-rp-grey">Pod {entry.pod_id}</td>
                  <td className="px-4 py-3 text-xs text-neutral-400">{entry.date}</td>
                  <td className="px-4 py-3">
                    <StarRating rating={entry.rating} />
                  </td>
                  <td className="px-4 py-3 text-sm text-neutral-300">{entry.comment}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <p className="text-xs text-neutral-500 mt-4">
        Showing mock data. Feedback API not yet connected.
      </p>
    </DashboardLayout>
  );
}
