"use client";

import { useEffect } from "react";

export default function RootError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    console.error("[RaceControl] Root error boundary:", error);
  }, [error]);

  return (
    <div className="min-h-screen bg-rp-black flex items-center justify-center">
      <div className="text-center space-y-4 p-8">
        <h2 className="text-xl font-bold text-white">Something went wrong</h2>
        <p className="text-sm text-neutral-400 max-w-md">
          {error.message || "An unexpected error occurred."}
        </p>
        <div className="flex gap-3 justify-center">
          <button
            onClick={reset}
            className="px-6 py-2.5 bg-rp-red text-white rounded-lg text-sm font-medium hover:bg-rp-red/80 transition-colors"
          >
            Try Again
          </button>
          <button
            onClick={() => (window.location.href = "/billing")}
            className="px-6 py-2.5 bg-rp-card border border-rp-border text-neutral-300 rounded-lg text-sm font-medium hover:bg-rp-card/80 transition-colors"
          >
            Go to Billing
          </button>
        </div>
      </div>
    </div>
  );
}
