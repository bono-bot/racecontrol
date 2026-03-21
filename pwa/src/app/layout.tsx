import type { Metadata, Viewport } from "next";
import { Montserrat } from "next/font/google";
import "./globals.css";
import RpToaster from "@/components/Toaster";

const montserrat = Montserrat({
  variable: "--font-montserrat",
  subsets: ["latin"],
  weight: ["300", "400", "500", "600", "700"],
});

export const metadata: Metadata = {
  title: "RacingPoint",
  description: "Your sim racing companion",
  manifest: "/manifest.json",
  appleWebApp: {
    capable: true,
    statusBarStyle: "black-translucent",
    title: "RacingPoint",
  },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  maximumScale: 1,
  userScalable: false,
  themeColor: "#1A1A1A",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark">
      <head>
        <script dangerouslySetInnerHTML={{ __html: `
          try {
            if (localStorage.getItem("rp_auth_v") !== "2") {
              localStorage.removeItem("rp_token");
              localStorage.setItem("rp_auth_v", "2");
            }
          } catch(e) {}
        `}} />
      </head>
      <body className={`${montserrat.variable} min-h-screen bg-rp-dark text-white antialiased font-sans`}>
        <RpToaster />
        {children}
      </body>
    </html>
  );
}
