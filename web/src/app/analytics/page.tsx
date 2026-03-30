"use client";

import DashboardLayout from "@/components/DashboardLayout";
import { EmptyState } from "@/components/Skeleton";
import { BarChart2 } from "lucide-react";

export default function AnalyticsPage() {
  return (
    <DashboardLayout>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white">Analytics</h1>
        <p className="text-sm text-rp-grey">Revenue, usage trends, and business intelligence</p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-6">
        <a
          href="/analytics/business"
          className="bg-rp-card border border-rp-border rounded-lg p-5 hover:border-rp-red transition-colors"
        >
          <h2 className="text-sm font-medium text-neutral-300 mb-1">Business Analytics</h2>
          <p className="text-xs text-rp-grey">Revenue, sessions, and customer metrics</p>
        </a>
        <a
          href="/analytics/ebitda"
          className="bg-rp-card border border-rp-border rounded-lg p-5 hover:border-rp-red transition-colors"
        >
          <h2 className="text-sm font-medium text-neutral-300 mb-1">EBITDA Dashboard</h2>
          <p className="text-xs text-rp-grey">Profit and loss breakdown</p>
        </a>
      </div>

      <EmptyState
        icon={<BarChart2 className="w-10 h-10" />}
        headline="Select a report above"
        hint="Choose a dashboard to view detailed analytics."
      />
    </DashboardLayout>
  );
}
