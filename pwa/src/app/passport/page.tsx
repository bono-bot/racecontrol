"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { PassportData, PassportTier, PassportTierItem } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

function formatLapTime(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${minutes}:${seconds.toString().padStart(2, "0")}.${millis
    .toString()
    .padStart(3, "0")}`;
}

function CollectionTile({ item }: { item: PassportTierItem }) {
  const secondaryText = item.driven
    ? item.best_lap_ms
      ? formatLapTime(item.best_lap_ms)
      : `${item.lap_count} laps`
    : item.category;

  return (
    <div
      className={`bg-rp-card border border-rp-border rounded-xl p-3 min-h-[44px] flex flex-col gap-1 ${
        item.driven ? "" : "opacity-30"
      }`}
    >
      <span className="text-xs text-white leading-tight">{item.name}</span>
      <span className="text-xs text-rp-grey leading-tight">{secondaryText}</span>
    </div>
  );
}

function TierSection({
  tier,
  label,
}: {
  tier: PassportTier;
  label: string;
}) {
  const percent =
    tier.target > 0 ? Math.round((tier.driven_count / tier.target) * 100) : 0;

  return (
    <div className="mb-6">
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm font-bold text-white">{label}</span>
        <span className="text-xs text-rp-grey">
          {tier.driven_count} / {tier.target}
        </span>
      </div>
      <div className="bg-rp-border rounded-full h-1 mb-3">
        <div
          className="bg-rp-red rounded-full h-1"
          style={{ width: `${percent}%` }}
        />
      </div>
      <div className="grid grid-cols-3 gap-2">
        {tier.items.map((item) => (
          <CollectionTile key={item.id} item={item} />
        ))}
      </div>
    </div>
  );
}

function PassportSection({
  title,
  tiers,
  tierLabels,
  otherItems,
  otherLabel,
}: {
  title: string;
  tiers: { starter: PassportTier; explorer: PassportTier; legend: PassportTier };
  tierLabels: { starter: string; explorer: string; legend: string };
  otherItems: PassportTierItem[];
  otherLabel: string;
}) {
  return (
    <div className="mb-8">
      <h2 className="text-xl font-bold text-white mb-4">{title}</h2>
      <TierSection tier={tiers.starter} label={tierLabels.starter} />
      <TierSection tier={tiers.explorer} label={tierLabels.explorer} />
      <TierSection tier={tiers.legend} label={tierLabels.legend} />
      {otherItems.length > 0 && (
        <div className="mb-6">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-bold text-white">{otherLabel}</span>
          </div>
          <div className="grid grid-cols-3 gap-2">
            {otherItems.map((item) => (
              <CollectionTile key={item.id} item={item} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export default function PassportPage() {
  const router = useRouter();
  const [data, setData] = useState<PassportData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    api.passport().then((res) => {
        if (res.error || !res.passport) {
          setError(true);
        } else {
          setData(res);
        }
        setLoading(false);
      })
      .catch(() => {
        setError(true);
        setLoading(false);
      });
  }, [router]);

  if (loading) {
    return (
      <div className="min-h-screen pb-20 flex items-center justify-center">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="min-h-screen pb-20 flex items-center justify-center">
        <p className="text-rp-grey text-sm text-center px-8">
          Could not load your passport. Pull to refresh.
        </p>
        <BottomNav />
      </div>
    );
  }

  if (!data?.passport) {
    return (
      <div className="min-h-screen pb-20 flex flex-col items-center justify-center gap-2">
        <p className="text-white font-bold text-lg">Your passport is empty</p>
        <p className="text-rp-grey text-sm text-center px-8">
          Complete a session to start your driving passport.
        </p>
        <BottomNav />
      </div>
    );
  }

  const { passport } = data;
  const { summary, tracks, cars } = passport;

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <h1 className="text-2xl font-bold text-white mb-6">My Passport</h1>

        {/* Summary card — 4-stat grid */}
        <div className="grid grid-cols-2 gap-3 mb-8">
          <div className="bg-rp-card border border-rp-border rounded-xl p-4">
            <p className="text-xs text-rp-grey mb-1">Tracks Driven</p>
            <p className="text-2xl font-bold text-white">
              {summary.unique_tracks}
            </p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded-xl p-4">
            <p className="text-xs text-rp-grey mb-1">Cars Driven</p>
            <p className="text-2xl font-bold text-white">
              {summary.unique_cars}
            </p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded-xl p-4">
            <p className="text-xs text-rp-grey mb-1">Total Laps</p>
            <p className="text-2xl font-bold text-white">
              {summary.total_laps}
            </p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded-xl p-4">
            <p className="text-xs text-rp-grey mb-1">Week Streak</p>
            <p className="text-2xl font-bold text-white">
              {summary.streak_weeks}
            </p>
          </div>
        </div>

        {/* Circuits section — Starter Circuits / Explorer Circuits / Legend Circuits */}
        <PassportSection
          title="Circuits"
          tiers={tracks.tiers}
          tierLabels={{
            starter: "Starter Circuits",
            explorer: "Explorer Circuits",
            legend: "Legend Circuits",
          }}
          otherItems={tracks.other || []}
          otherLabel="Other Circuits"
        />

        {/* Cars section — Starter Garage / Explorer Garage / Legend Garage */}
        <PassportSection
          title="Cars"
          tiers={cars.tiers}
          tierLabels={{
            starter: "Starter Garage",
            explorer: "Explorer Garage",
            legend: "Legend Garage",
          }}
          otherItems={cars.other || []}
          otherLabel="Other Cars"
        />
      </div>
      <BottomNav />
    </div>
  );
}
