import type { Metadata } from "next";
import { Chakra_Petch, Montserrat, JetBrains_Mono } from "next/font/google";
import "./globals.css";

// Layer 1 Foundation — 3-font system per CAPTAIN-RATIFIED §3.2 (2026-05-06).
// Display = Chakra Petch (Enthocentric stand-in, F1-broadcast face).
// Body = Montserrat. Mono = JetBrains Mono (lap times, IDs, money).
//
// CSS vars exposed: --font-display-google / --font-body-google /
// --font-mono-google. Consumed in globals.css @theme as fallback chain
// roots for --font-display / --font-body / --font-mono.

const display = Chakra_Petch({
  subsets: ["latin"],
  weight: ["500", "600", "700"],
  variable: "--font-display-google",
  display: "swap",
});

const body = Montserrat({
  subsets: ["latin"],
  weight: ["400", "500", "600", "700"],
  variable: "--font-body-google",
  display: "swap",
});

const mono = JetBrains_Mono({
  subsets: ["latin"],
  weight: ["400", "500", "600"],
  variable: "--font-mono-google",
  display: "swap",
});

export const metadata: Metadata = {
  title: "RacingPoint V2",
  description: "RacingPoint V2 — dedicated Next.js host (Phase 0.1 substrate, Layer 1 Foundation)",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className={`${display.variable} ${body.variable} ${mono.variable}`}>
      <body>{children}</body>
    </html>
  );
}
