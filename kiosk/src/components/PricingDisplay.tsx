"use client";

import { useState, useEffect } from "react";

const API_BASE =
  process.env.NEXT_PUBLIC_API_URL ||
  (typeof window !== "undefined"
    ? `${window.location.protocol}//${window.location.host}`
    : "http://localhost:8080");

interface DisplayTier {
  id: string;
  name: string;
  duration_minutes: number;
  base_price_paise: number;
  dynamic_price_paise: number;
  has_discount: boolean;
  is_trial: boolean;
  sort_order: number;
}

interface Props {
  onSelectTier: (tier: {
    id: string;
    name: string;
    duration_minutes: number;
    price_paise: number;
    is_trial: boolean;
    is_active: boolean;
  }) => void;
  hasUsedTrial?: boolean;
}

export default function PricingDisplay({ onSelectTier, hasUsedTrial }: Props) {
  const [tiers, setTiers] = useState<DisplayTier[]>([]);

  useEffect(() => {
    let active = true;
    const load = () => {
      fetch(`${API_BASE}/api/v1/pricing/display`)
        .then((r) => r.json())
        .then((d) => {
          if (active && d?.tiers) setTiers(d.tiers);
        })
        .catch(() => {});
    };
    load();
    const id = setInterval(load, 30000);
    return () => { active = false; clearInterval(id); };
  }, []);

  if (!tiers.length) return null;

  const paidTiers = tiers.filter((t) => !t.is_trial);
  const trialTier = tiers.find((t) => t.is_trial);
  const showTrial = trialTier && !hasUsedTrial;
  const mostPopularIndex = paidTiers.length === 3 ? 1 : Math.floor(paidTiers.length / 2);

  return (
    <div data-testid="pricing-display" className="space-y-3">
      <div className="flex gap-3 items-end justify-center">
        {paidTiers.map((tier, idx) => {
          const isPopular = idx === mostPopularIndex && paidTiers.length >= 3;
          return (
            <button
              key={tier.id}
              data-testid={`pricing-tier-${tier.id}`}
              onClick={() =>
                onSelectTier({
                  id: tier.id,
                  name: tier.name,
                  duration_minutes: tier.duration_minutes,
                  price_paise: tier.dynamic_price_paise,
                  is_trial: false,
                  is_active: true,
                })
              }
              className={`relative flex-1 rounded-xl border-2 p-4 text-center transition-all ${
                isPopular
                  ? "border-[#E10600] scale-105 bg-[#E10600]/5 shadow-lg shadow-[#E10600]/20"
                  : "border-rp-border bg-rp-surface hover:border-rp-red/50"
              }`}
            >
              {isPopular && (
                <span className="absolute -top-3 left-1/2 -translate-x-1/2 bg-[#E10600] text-white text-xs font-bold px-3 py-1 rounded-full whitespace-nowrap">
                  Most Popular
                </span>
              )}
              <p className="text-sm font-semibold text-white mt-1">{tier.name}</p>
              <p className="text-xs text-rp-grey">{tier.duration_minutes} min</p>
              <div className="mt-2">
                {tier.has_discount && (
                  <span className="line-through text-rp-grey text-xs mr-1">
                    {(tier.base_price_paise / 100).toFixed(0)}
                  </span>
                )}
                <span
                  className={`text-lg font-bold ${
                    isPopular ? "text-[#E10600]" : "text-white"
                  }`}
                >
                  {(tier.dynamic_price_paise / 100).toFixed(0)} credits
                </span>
              </div>
            </button>
          );
        })}
      </div>
      {showTrial && (
        <button
          data-testid="pricing-trial"
          onClick={() =>
            onSelectTier({
              id: trialTier.id,
              name: trialTier.name,
              duration_minutes: trialTier.duration_minutes,
              price_paise: 0,
              is_trial: true,
              is_active: true,
            })
          }
          className="w-full py-2 text-center text-sm text-rp-grey border border-dashed border-rp-border rounded-lg hover:border-rp-red/50 transition-colors"
        >
          Try for Free ({trialTier.duration_minutes} min trial)
        </button>
      )}
    </div>
  );
}
