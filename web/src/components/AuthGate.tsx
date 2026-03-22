"use client";
import { useEffect, useState } from "react";
import { useRouter, usePathname } from "next/navigation";
import { isAuthenticated } from "@/lib/auth";
import { useIdleTimeout } from "@/hooks/useIdleTimeout";

// Pages accessible without PIN login
const PUBLIC_ROUTES = ["/login", "/cameras", "/cameras/playback"];

export function AuthGate({ children }: { children: React.ReactNode }) {
  const [hydrated, setHydrated] = useState(false);
  const router = useRouter();
  const pathname = usePathname();

  useIdleTimeout(15 * 60 * 1000); // 15 minutes

  const isPublic = PUBLIC_ROUTES.includes(pathname);

  useEffect(() => {
    setHydrated(true);
    if (!isAuthenticated() && !isPublic) {
      router.push("/login");
    }
  }, [pathname, router, isPublic]);

  if (!hydrated) return null;
  if (!isAuthenticated() && !isPublic) return null;

  return <>{children}</>;
}
