import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "RacingPoint V2",
  description: "RacingPoint V2 — dedicated Next.js host (Phase 0.1 substrate)",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
