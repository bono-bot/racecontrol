"use client";

import { useEffect, useState, useMemo } from "react";
import { api } from "@/lib/api";
import type { Driver, PricingTier } from "@/lib/api";

interface BillingStartModalProps {
  podId: string;
  podName: string;
  onClose: () => void;
  onStart: (data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    custom_price_paise?: number;
    custom_duration_minutes?: number;
  }) => void;
}

const formatINR = (paise: number) =>
  new Intl.NumberFormat("en-IN", {
    style: "currency",
    currency: "INR",
  }).format(paise / 100);

export default function BillingStartModal({
  podId,
  podName,
  onClose,
  onStart,
}: BillingStartModalProps) {
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [tiers, setTiers] = useState<PricingTier[]>([]);
  const [loading, setLoading] = useState(true);

  const [driverSearch, setDriverSearch] = useState("");
  const [selectedDriver, setSelectedDriver] = useState<Driver | null>(null);
  const [selectedTier, setSelectedTier] = useState<PricingTier | null>(null);

  const [variableTime, setVariableTime] = useState(false);
  const [customMinutes, setCustomMinutes] = useState(30);
  const [customPriceRupees, setCustomPriceRupees] = useState(200);

  const [starting, setStarting] = useState(false);

  useEffect(() => {
    Promise.all([api.listDrivers(), api.listPricingTiers()])
      .then(([driverRes, tierRes]) => {
        setDrivers(driverRes.drivers || []);
        setTiers((tierRes.tiers || []).filter((t) => t.is_active));
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  const filteredDrivers = useMemo(() => {
    if (!driverSearch.trim()) return drivers.slice(0, 10);
    const q = driverSearch.toLowerCase();
    return drivers
      .filter(
        (d) =>
          d.name.toLowerCase().includes(q) ||
          d.email?.toLowerCase().includes(q) ||
          d.phone?.includes(q)
      )
      .slice(0, 10);
  }, [driverSearch, drivers]);

  const activeTiers = useMemo(
    () => tiers.sort((a, b) => (a.sort_order ?? 0) - (b.sort_order ?? 0)),
    [tiers]
  );

  const canStart = selectedDriver && (selectedTier || variableTime);

  function handleStart() {
    if (!selectedDriver) return;
    setStarting(true);

    const data: {
      pod_id: string;
      driver_id: string;
      pricing_tier_id: string;
      custom_price_paise?: number;
      custom_duration_minutes?: number;
    } = {
      pod_id: podId,
      driver_id: selectedDriver.id,
      pricing_tier_id: selectedTier?.id || "",
    };

    if (variableTime) {
      data.custom_duration_minutes = customMinutes;
      data.custom_price_paise = customPriceRupees * 100;
    }

    onStart(data);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-lg bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl p-6 mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-lg font-bold text-zinc-100">Start Session</h2>
            <p className="text-sm text-zinc-500">{podName}</p>
          </div>
          <button
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-300 transition-colors text-xl leading-none"
          >
            &times;
          </button>
        </div>

        {loading ? (
          <div className="text-center py-8 text-zinc-500 text-sm">
            Loading...
          </div>
        ) : (
          <div className="space-y-6">
            {/* Driver Selection */}
            <div>
              <label className="block text-sm font-medium text-zinc-300 mb-2">
                Driver
              </label>
              {selectedDriver ? (
                <div className="flex items-center justify-between bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2">
                  <div className="flex items-center gap-3">
                    <div className="w-8 h-8 rounded-full bg-orange-500/20 flex items-center justify-center text-orange-400 font-bold text-sm">
                      {selectedDriver.name.charAt(0).toUpperCase()}
                    </div>
                    <div>
                      <div className="text-sm text-zinc-200">
                        {selectedDriver.name}
                      </div>
                      {selectedDriver.phone && (
                        <div className="text-xs text-zinc-500">
                          {selectedDriver.phone}
                        </div>
                      )}
                    </div>
                  </div>
                  <button
                    onClick={() => {
                      setSelectedDriver(null);
                      setDriverSearch("");
                    }}
                    className="text-xs text-zinc-500 hover:text-zinc-300"
                  >
                    Change
                  </button>
                </div>
              ) : (
                <div>
                  <input
                    type="text"
                    placeholder="Search by name, email, or phone..."
                    value={driverSearch}
                    onChange={(e) => setDriverSearch(e.target.value)}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-orange-500 transition-colors"
                    autoFocus
                  />
                  {filteredDrivers.length > 0 && (
                    <div className="mt-1 bg-zinc-800 border border-zinc-700 rounded-lg overflow-hidden max-h-48 overflow-y-auto">
                      {filteredDrivers.map((driver) => (
                        <button
                          key={driver.id}
                          onClick={() => setSelectedDriver(driver)}
                          className="w-full flex items-center gap-3 px-3 py-2 hover:bg-zinc-700/50 transition-colors text-left"
                        >
                          <div className="w-7 h-7 rounded-full bg-orange-500/20 flex items-center justify-center text-orange-400 font-bold text-xs">
                            {driver.name.charAt(0).toUpperCase()}
                          </div>
                          <div>
                            <div className="text-sm text-zinc-200">
                              {driver.name}
                            </div>
                            <div className="text-xs text-zinc-500">
                              {driver.email || driver.phone || "No contact"}
                            </div>
                          </div>
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Pricing Tiers */}
            <div>
              <label className="block text-sm font-medium text-zinc-300 mb-2">
                Session Type
              </label>
              <div className="grid grid-cols-2 gap-2">
                {activeTiers.map((tier) => {
                  const isSelected =
                    !variableTime && selectedTier?.id === tier.id;
                  return (
                    <button
                      key={tier.id}
                      onClick={() => {
                        setSelectedTier(tier);
                        setVariableTime(false);
                      }}
                      className={`relative rounded-lg border p-3 text-left transition-all ${
                        isSelected
                          ? "border-orange-500 bg-orange-500/10"
                          : "border-zinc-700 bg-zinc-800 hover:border-zinc-600"
                      }`}
                    >
                      {tier.is_trial && (
                        <span className="absolute top-2 right-2 bg-emerald-500/20 text-emerald-400 text-[10px] font-bold px-1.5 py-0.5 rounded">
                          FREE
                        </span>
                      )}
                      <div className="text-sm font-medium text-zinc-200">
                        {tier.name}
                      </div>
                      <div className="text-xs text-zinc-500 mt-0.5">
                        {tier.duration_minutes} min
                      </div>
                      <div className="text-sm font-bold text-orange-400 mt-1">
                        {tier.is_trial
                          ? "Free"
                          : formatINR(tier.price_paise)}
                      </div>
                    </button>
                  );
                })}

                {/* Variable Time button */}
                <button
                  onClick={() => {
                    setVariableTime(true);
                    setSelectedTier(null);
                  }}
                  className={`rounded-lg border p-3 text-left transition-all ${
                    variableTime
                      ? "border-orange-500 bg-orange-500/10"
                      : "border-zinc-700 bg-zinc-800 hover:border-zinc-600"
                  }`}
                >
                  <div className="text-sm font-medium text-zinc-200">
                    Variable Time
                  </div>
                  <div className="text-xs text-zinc-500 mt-0.5">
                    Custom duration
                  </div>
                  <div className="text-sm font-bold text-orange-400 mt-1">
                    Custom
                  </div>
                </button>
              </div>
            </div>

            {/* Variable Time inputs */}
            {variableTime && (
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs text-zinc-400 mb-1">
                    Duration (minutes)
                  </label>
                  <input
                    type="number"
                    min={5}
                    max={480}
                    value={customMinutes}
                    onChange={(e) =>
                      setCustomMinutes(parseInt(e.target.value) || 30)
                    }
                    className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-orange-500 transition-colors"
                  />
                </div>
                <div>
                  <label className="block text-xs text-zinc-400 mb-1">
                    Price (INR)
                  </label>
                  <input
                    type="number"
                    min={0}
                    step={50}
                    value={customPriceRupees}
                    onChange={(e) =>
                      setCustomPriceRupees(parseInt(e.target.value) || 0)
                    }
                    className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-orange-500 transition-colors"
                  />
                </div>
              </div>
            )}

            {/* Start Button */}
            <button
              onClick={handleStart}
              disabled={!canStart || starting}
              className={`w-full rounded-lg py-3 font-semibold text-sm transition-all ${
                canStart && !starting
                  ? "bg-orange-500 text-white hover:bg-orange-600 active:bg-orange-700"
                  : "bg-zinc-800 text-zinc-600 cursor-not-allowed"
              }`}
            >
              {starting ? "Starting..." : "Start Session"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
