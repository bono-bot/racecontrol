"use client";

import { ToastProvider } from "./Toast";

export default function AppShell({ children }: { children: React.ReactNode }) {
  return <ToastProvider>{children}</ToastProvider>;
}
