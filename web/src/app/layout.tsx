import type { Metadata } from "next";
import { Montserrat } from "next/font/google";
import "./globals.css";
import { AuthGate } from "@/components/AuthGate";

const montserrat = Montserrat({
  variable: "--font-montserrat",
  subsets: ["latin"],
  weight: ["300", "400", "500", "600", "700"],
});

export const metadata: Metadata = {
  title: "RaceControl — RacingPoint Bandlaguda",
  description: "Sim racing venue management system",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="dark">
      <body
        className={`${montserrat.variable} antialiased bg-rp-black text-white font-sans`}
      >
        <AuthGate>{children}</AuthGate>
      </body>
    </html>
  );
}
