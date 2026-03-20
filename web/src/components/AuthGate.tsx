"use client";
import { useEffect, useState } from "react";
import { useRouter, usePathname } from "next/navigation";
import { isAuthenticated } from "@/lib/auth";
import { useIdleTimeout } from "@/hooks/useIdleTimeout";

export function AuthGate({ children }: { children: React.ReactNode }) {
  const [hydrated, setHydrated] = useState(false);
  const router = useRouter();
  const pathname = usePathname();

  useIdleTimeout(15 * 60 * 1000); // 15 minutes

  useEffect(() => {
    setHydrated(true);
    if (!isAuthenticated() && pathname !== "/login") {
      router.push("/login");
    }
  }, [pathname, router]);

  if (!hydrated) return null;
  if (!isAuthenticated() && pathname !== "/login") return null;

  return <>{children}</>;
}
