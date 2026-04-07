"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";

/** Redirect /spectator to /spectator/circuit */
export default function SpectatorIndexPage() {
  const router = useRouter();
  useEffect(() => {
    router.replace("/spectator/circuit");
  }, [router]);
  return (
    <div className="min-h-screen bg-[#0A0A0A] flex items-center justify-center">
      <p className="text-neutral-500 text-sm">Redirecting to circuit viewer...</p>
    </div>
  );
}
