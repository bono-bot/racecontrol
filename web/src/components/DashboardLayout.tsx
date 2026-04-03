"use client";

import { useRouter, usePathname } from "next/navigation";
import Sidebar from "./Sidebar";
import AiChatPanel from "./AiChatPanel";
import AppShell from "./AppShell";

function BackButton() {
  const router = useRouter();
  const pathname = usePathname();

  // Don't show on home or POS default page
  if (pathname === "/" || pathname === "/billing") return null;

  // For sub-pages like /billing/pricing, go to parent; otherwise go back
  const parentMap: Record<string, string> = {
    "/billing/pricing": "/billing",
    "/billing/history": "/billing",
    "/games/reliability": "/games",
  };
  const target = parentMap[pathname];

  return (
    <button
      onClick={() => (target ? router.push(target) : router.back())}
      className="flex items-center gap-2 px-3 py-2 mb-4 text-sm text-neutral-400 hover:text-white hover:bg-rp-card rounded-lg transition-colors"
      aria-label="Go back"
    >
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
      </svg>
      Back
    </button>
  );
}

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return (
    <AppShell>
      <div className="flex min-h-screen">
        <Sidebar />
        <main className="flex-1 overflow-auto">
          <div className="p-6">
            <BackButton />
            {children}
          </div>
        </main>
        <AiChatPanel />
      </div>
    </AppShell>
  );
}
