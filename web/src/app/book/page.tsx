"use client";

import { useState, useEffect } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

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

interface PodStatus {
  pod_number: number;
  ws_connected: boolean;
  http_reachable: boolean;
}

interface SocialProof {
  drivers_this_week: number;
  sessions_today: number;
}

export default function BookPage() {
  const [tiers, setTiers] = useState<DisplayTier[]>([]);
  const [fleet, setFleet] = useState<PodStatus[]>([]);
  const [social, setSocial] = useState<SocialProof | null>(null);

  // Fetch pricing tiers (30s refresh)
  useEffect(() => {
    let active = true;
    const load = () => {
      fetch(`${API_BASE}/api/v1/pricing/display`)
        .then((r) => r.json())
        .then((d) => { if (active && d?.tiers) setTiers(d.tiers); })
        .catch(() => {});
    };
    load();
    const id = setInterval(load, 30000);
    return () => { active = false; clearInterval(id); };
  }, []);

  // Fetch fleet health (10s refresh)
  useEffect(() => {
    let active = true;
    const load = () => {
      fetch(`${API_BASE}/api/v1/fleet/health`)
        .then((r) => r.json())
        .then((d) => { if (active && Array.isArray(d?.pods)) setFleet(d.pods); })
        .catch(() => {});
    };
    load();
    const id = setInterval(load, 10000);
    return () => { active = false; clearInterval(id); };
  }, []);

  // Fetch social proof (60s refresh)
  useEffect(() => {
    let active = true;
    const load = () => {
      fetch(`${API_BASE}/api/v1/pricing/social-proof`)
        .then((r) => r.json())
        .then((d) => { if (active && d) setSocial(d); })
        .catch(() => {});
    };
    load();
    const id = setInterval(load, 60000);
    return () => { active = false; clearInterval(id); };
  }, []);

  const paidTiers = tiers.filter((t) => !t.is_trial);
  const trialTier = tiers.find((t) => t.is_trial);
  const mostPopularIndex = paidTiers.length === 3 ? 1 : Math.floor(paidTiers.length / 2);

  // Scarcity
  const available = fleet.filter((p) => p.ws_connected && p.http_reachable).length;
  const total = fleet.length || 8;
  const scarcityColor =
    available >= 5 ? "text-green-400" : available >= 2 ? "text-yellow-400" : "text-[#E10600]";
  const scarcityBg =
    available >= 5
      ? "bg-green-400/10 border-green-400/30"
      : available >= 2
        ? "bg-yellow-400/10 border-yellow-400/30"
        : "bg-[#E10600]/10 border-[#E10600]/30";

  return (
    <div className="min-h-screen bg-[#1A1A1A] text-white">
      <div className="max-w-2xl mx-auto px-4 py-12">
        {/* Header */}
        <h1
          className="text-3xl font-bold text-center mb-2"
          style={{ fontFamily: "Montserrat, sans-serif" }}
        >
          Book Your Session
        </h1>
        <p className="text-center text-[#5A5A5A] mb-8">Choose your racing experience</p>

        {/* Scarcity Banner */}
        {fleet.length > 0 && (
          <div
            data-testid="scarcity-banner"
            className={`rounded-lg border px-4 py-3 text-center text-sm mb-6 ${scarcityBg}`}
          >
            {available === 0 ? (
              <span className={scarcityColor}>
                All pods in use — next slot likely in ~30min
              </span>
            ) : (
              <span className={scarcityColor}>
                {available} of {total} pods available now
              </span>
            )}
          </div>
        )}

        {/* Pricing Tiers — Anchoring Layout */}
        {paidTiers.length > 0 && (
          <div
            data-testid="pricing-display"
            className="flex gap-4 items-end justify-center mb-6"
          >
            {paidTiers.map((tier, idx) => {
              const isPopular = idx === mostPopularIndex && paidTiers.length >= 3;
              return (
                <div
                  key={tier.id}
                  data-testid={`pricing-tier-${tier.id}`}
                  className={`relative flex-1 rounded-xl border-2 p-6 text-center transition-all ${
                    isPopular
                      ? "border-[#E10600] scale-105 bg-[#E10600]/5 shadow-lg shadow-[#E10600]/20"
                      : "border-[#333333] bg-[#222222] hover:border-[#E10600]/50"
                  }`}
                >
                  {isPopular && (
                    <span className="absolute -top-3 left-1/2 -translate-x-1/2 bg-[#E10600] text-white text-xs font-bold px-3 py-1 rounded-full whitespace-nowrap">
                      Most Popular
                    </span>
                  )}
                  <p className="text-lg font-semibold text-white mt-1">{tier.name}</p>
                  <p className="text-sm text-[#5A5A5A]">{tier.duration_minutes} min</p>
                  <div className="mt-3">
                    {tier.has_discount && (
                      <span className="line-through text-[#5A5A5A] text-sm mr-1">
                        {"\u20B9"}
                        {(tier.base_price_paise / 100).toFixed(0)}
                      </span>
                    )}
                    <span
                      className={`text-2xl font-bold ${
                        isPopular ? "text-[#E10600]" : "text-white"
                      }`}
                    >
                      {"\u20B9"}
                      {(tier.dynamic_price_paise / 100).toFixed(0)}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {/* Trial CTA */}
        {trialTier && (
          <div className="text-center mb-8">
            <p className="text-sm text-[#5A5A5A]">
              First time? Try a {trialTier.duration_minutes}-min free trial — no commitment
            </p>
          </div>
        )}

        {/* Social Proof Bar */}
        <div
          data-testid="social-proof-bar"
          className="rounded-lg border border-[#333333] bg-[#222222] px-4 py-3 flex justify-around text-center"
        >
          <div>
            <p className="text-lg font-bold text-white">{social?.drivers_this_week ?? 0}</p>
            <p className="text-xs text-[#5A5A5A]">
              {social?.drivers_this_week === 0 ? "Be the first this week!" : "drivers this week"}
            </p>
          </div>
          <div className="border-l border-[#333333]" />
          <div>
            <p className="text-lg font-bold text-white">{social?.sessions_today ?? 0}</p>
            <p className="text-xs text-[#5A5A5A]">
              {social?.sessions_today === 0 ? "Be the first today!" : "sessions today"}
            </p>
          </div>
        </div>

        {/* Walk-in CTA */}
        <p className="text-center text-sm text-[#5A5A5A] mt-8">
          Walk in or call us to book your session
        </p>
      </div>
    </div>
  );
}
