"use client";

import { useEffect } from "react";

export default function BillingError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    console.error("[RaceControl] Billing error:", error);
  }, [error]);

  return (
    <div className="min-h-screen bg-rp-black flex items-center justify-center">
      <div className="text-center space-y-4 p-8">
        <h2 className="text-xl font-bold text-white">Billing Error</h2>
        <p className="text-sm text-neutral-400 max-w-md">
          {error.message || "Failed to load billing page."}
        </p>
        <button
          onClick={reset}
          className="px-6 py-2.5 bg-rp-red text-white rounded-lg text-sm font-medium hover:bg-rp-red/80 transition-colors"
        >
          Retry
        </button>
      </div>
    </div>
  );
}
