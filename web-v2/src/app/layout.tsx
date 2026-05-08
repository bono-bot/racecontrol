import type { Metadata } from "next";
import { Montserrat } from "next/font/google";
import "./globals.css";

// Brand body font per `racecontrol/CLAUDE.md` Brand Identity. Exposes
// CSS var --rp-font-body consumed by globals.css + ManagerPill.module.css.
// Loaded via next/font/google so production bundles inline the font files
// (no FOUT, no third-party network). Wave 0 K2 close-out (Session 7 UI-REVIEW
// FLAG-2). Headers font (Enthocentric, custom) remains a separate concern.
const montserrat = Montserrat({
  subsets: ["latin"],
  variable: "--rp-font-body",
  weight: ["400", "500", "600", "700"],
  display: "swap",
});

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
    <html lang="en" className={montserrat.variable}>
      <body>{children}</body>
    </html>
  );
}
