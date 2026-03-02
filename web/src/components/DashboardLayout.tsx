"use client";

import Sidebar from "./Sidebar";
import AiChatPanel from "./AiChatPanel";

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-screen">
      <Sidebar />
      <main className="flex-1 overflow-auto">
        <div className="p-6">{children}</div>
      </main>
      <AiChatPanel />
    </div>
  );
}
