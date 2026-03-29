"use client";
import { useEffect, useState, useRef } from "react";
import { useRouter, usePathname } from "next/navigation";
import { isAuthenticated } from "@/lib/auth";
import { useIdleTimeout } from "@/hooks/useIdleTimeout";

// Pages accessible without PIN login
const PUBLIC_ROUTES = ["/login", "/cameras", "/cameras/playback"];

function AuthLoadingSkeleton() {
  return (
    <div className="min-h-screen bg-rp-black flex items-center justify-center">
      <div className="text-center space-y-3">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin mx-auto" />
        <p className="text-sm text-neutral-500">Loading...</p>
      </div>
    </div>
  );
}

export function AuthGate({ children }: { children: React.ReactNode }) {
  const [hydrated, setHydrated] = useState(false);
  const redirectingRef = useRef(false);
  const router = useRouter();
  const pathname = usePathname();

  useIdleTimeout(15 * 60 * 1000); // 15 minutes

  const isPublic = PUBLIC_ROUTES.includes(pathname);

  useEffect(() => {
    setHydrated(true);
    if (!isAuthenticated() && !isPublic && !redirectingRef.current) {
      redirectingRef.current = true;
      router.push("/login");
    }
  }, [pathname, router, isPublic]);

  // Reset redirect flag when we land on login
  useEffect(() => {
    if (pathname === "/login") {
      redirectingRef.current = false;
    }
  }, [pathname]);

  if (!hydrated) return <AuthLoadingSkeleton />;
  if (!isAuthenticated() && !isPublic) return <AuthLoadingSkeleton />;

  return <>{children}</>;
}
