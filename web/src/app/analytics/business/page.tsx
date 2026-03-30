"use client";

import DashboardLayout from "@/components/DashboardLayout";
import Link from "next/link";

export default function BusinessPage() {
  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Business Analytics</h1>
          <p className="text-sm text-rp-grey">Financial overview and business metrics</p>
        </div>
      </div>

      <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
        <p className="text-neutral-400 mb-4">
          View detailed EBITDA analysis, revenue breakdown, and expense tracking.
        </p>
        <Link
          href="/analytics/ebitda"
          className="inline-block px-6 py-3 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded-lg transition-colors"
        >
          View Financial Dashboard
        </Link>
      </div>
    </DashboardLayout>
  );
}
