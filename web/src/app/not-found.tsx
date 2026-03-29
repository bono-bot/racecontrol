"use client";

import { useEffect, useRef } from "react";

export default function NotFound() {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    // Auto-redirect to billing after 3 seconds (POS kiosk recovery)
    timerRef.current = setTimeout(() => {
      window.location.href = "/billing";
    }, 3000);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  const handleClick = () => {
    if (timerRef.current) clearTimeout(timerRef.current);
    window.location.href = "/billing";
  };

  return (
    <div className="min-h-screen bg-rp-black flex items-center justify-center">
      <div className="text-center space-y-4 p-8">
        <h1 className="text-4xl font-bold text-rp-red">404</h1>
        <p className="text-sm text-neutral-400">
          Page not found. Redirecting to Billing...
        </p>
        <button
          onClick={handleClick}
          className="px-6 py-2.5 bg-rp-red text-white rounded-lg text-sm font-medium hover:bg-rp-red/80 transition-colors"
        >
          Go to Billing Now
        </button>
      </div>
    </div>
  );
}
