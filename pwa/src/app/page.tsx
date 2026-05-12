"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { isLoggedIn } from "@/lib/api";
import { BrandStrip } from "@/components/landing/BrandStrip";
import { HeroVisual } from "@/components/landing/HeroVisual";
import { HowItWorks } from "@/components/landing/HowItWorks";
import { Footer } from "@/components/landing/Footer";

export default function RootPage() {
  const router = useRouter();
  const [authChecked, setAuthChecked] = useState(false);

  useEffect(() => {
    if (isLoggedIn()) {
      router.replace("/dashboard");
      return;
    }
    setAuthChecked(true);
  }, [router]);

  if (!authChecked) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-rp-dark">
        <div className="text-center">
          <div className="mb-2 font-display text-3xl font-bold text-rp-red">RP</div>
          <p className="text-sm text-rp-grey">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <main className="min-h-screen bg-rp-dark">
      <BrandStrip />

      <section className="flex flex-col px-5 pt-8 pb-6 sm:px-8 sm:pt-12">
        <h1 className="font-display text-3xl font-bold leading-tight text-white sm:text-5xl">
          Real cars. Real circuits. Real wheel-to-wheel.
        </h1>
        <p className="mt-3 text-base text-rp-grey sm:mt-4 sm:text-lg">
          Hyderabad&apos;s first competitive sim racing venue.
        </p>

        <div className="mt-6 sm:mt-8">
          <HeroVisual />
        </div>

        <div className="mt-6 grid grid-cols-1 gap-3 sm:mt-8 sm:grid-cols-2 sm:gap-4">
          <Link
            href="/register"
            className="flex h-12 items-center justify-center rounded-md bg-rp-red font-semibold text-white transition active:bg-rp-red-hover sm:h-14"
          >
            Register
          </Link>
          <Link
            href="/login"
            className="flex h-12 items-center justify-center rounded-md border border-rp-border text-white transition active:bg-rp-card sm:h-14"
          >
            Sign in
          </Link>
        </div>

        <p className="mt-4 text-center text-xs text-rp-grey sm:mt-6">
          12 drivers racing now · 247 laps this week
        </p>
      </section>

      <HowItWorks />
      <Footer />
    </main>
  );
}
