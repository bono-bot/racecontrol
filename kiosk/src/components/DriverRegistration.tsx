"use client";

import { useState, useEffect } from "react";
import { api } from "@/lib/api";
import type { Driver, PricingTier } from "@/lib/types";

interface DriverRegistrationProps {
  podId: string;
  onAssign: (data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    auth_type: string;
  }) => void;
  onCancel: () => void;
}

export function DriverRegistration({ podId, onAssign, onCancel }: DriverRegistrationProps) {
  const [step, setStep] = useState<"driver" | "tier">("driver");
  const [driverName, setDriverName] = useState("");
  const [driverPhone, setDriverPhone] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [tiers, setTiers] = useState<PricingTier[]>([]);
  const [selectedDriver, setSelectedDriver] = useState<Driver | null>(null);

  useEffect(() => {
    api.listDrivers().then((res) => setDrivers(res.drivers || []));
    api.listPricingTiers().then((res) => setTiers((res.tiers || []).filter((t) => t.is_active)));
  }, []);

  const filteredDrivers = searchQuery.length >= 2
    ? drivers.filter(
        (d) =>
          d.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
          (d.phone && d.phone.includes(searchQuery))
      )
    : [];

  async function handleCreateDriver() {
    if (!driverName.trim()) return;
    const result = await api.createDriver({
      name: driverName.trim(),
      phone: driverPhone.trim() || undefined,
    });
    if (result.id) {
      setSelectedDriver({ id: result.id, name: result.name, total_laps: 0, total_time_ms: 0 });
      setStep("tier");
    }
  }

  function handleSelectDriver(driver: Driver) {
    setSelectedDriver(driver);
    setStep("tier");
  }

  function handleSelectTier(tier: PricingTier) {
    if (!selectedDriver) return;
    onAssign({
      pod_id: podId,
      driver_id: selectedDriver.id,
      pricing_tier_id: tier.id,
      auth_type: "pin",
    });
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-rp-card border border-rp-border rounded-lg w-full max-w-md shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3 border-b border-rp-border">
          <h2 className="text-sm font-semibold">
            {step === "driver" ? "Register Driver" : "Select Plan"} — Pod {podId.replace(/\D/g, "")}
          </h2>
          <button onClick={onCancel} className="text-rp-grey hover:text-white text-lg">&times;</button>
        </div>

        <div className="p-5">
          {/* Step 1: Driver Selection */}
          {step === "driver" && (
            <div className="space-y-4">
              {/* Search existing */}
              <div>
                <label className="text-xs text-rp-grey uppercase tracking-wider block mb-1">
                  Search Existing Driver
                </label>
                <input
                  type="text"
                  placeholder="Name or phone..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="w-full px-3 py-2 bg-rp-surface border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
                />
                {filteredDrivers.length > 0 && (
                  <div className="mt-1 max-h-32 overflow-y-auto border border-rp-border rounded bg-rp-surface">
                    {filteredDrivers.slice(0, 5).map((d) => (
                      <button
                        key={d.id}
                        onClick={() => handleSelectDriver(d)}
                        className="w-full text-left px-3 py-2 hover:bg-rp-red/10 text-sm flex justify-between"
                      >
                        <span>{d.name}</span>
                        <span className="text-rp-grey text-xs">{d.total_laps} laps</span>
                      </button>
                    ))}
                  </div>
                )}
              </div>

              <div className="flex items-center gap-3 text-xs text-rp-grey">
                <div className="flex-1 h-px bg-rp-border" />
                <span>or create new</span>
                <div className="flex-1 h-px bg-rp-border" />
              </div>

              {/* New driver form */}
              <div className="space-y-3">
                <input
                  type="text"
                  placeholder="Driver name *"
                  value={driverName}
                  onChange={(e) => setDriverName(e.target.value)}
                  className="w-full px-3 py-2 bg-rp-surface border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
                />
                <input
                  type="text"
                  placeholder="Phone (optional)"
                  value={driverPhone}
                  onChange={(e) => setDriverPhone(e.target.value)}
                  className="w-full px-3 py-2 bg-rp-surface border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
                />
                <button
                  onClick={handleCreateDriver}
                  disabled={!driverName.trim()}
                  className="w-full py-2.5 bg-rp-red hover:bg-rp-red-hover disabled:opacity-40 disabled:cursor-not-allowed text-white font-semibold rounded text-sm transition-colors"
                >
                  Continue
                </button>
              </div>
            </div>
          )}

          {/* Step 2: Pricing Tier */}
          {step === "tier" && selectedDriver && (
            <div className="space-y-4">
              <div className="text-center mb-2">
                <p className="text-sm text-rp-grey">Driver</p>
                <p className="text-white font-semibold">{selectedDriver.name}</p>
              </div>

              {/* Tiers */}
              <div className="space-y-2">
                {tiers.map((tier) => (
                  <button
                    key={tier.id}
                    onClick={() => handleSelectTier(tier)}
                    className="w-full flex items-center justify-between px-4 py-3 bg-rp-surface border border-rp-border rounded hover:border-rp-red/50 transition-colors"
                  >
                    <div className="text-left">
                      <p className="text-sm font-semibold text-white">{tier.name}</p>
                      <p className="text-xs text-rp-grey">{tier.duration_minutes} min</p>
                    </div>
                    <span className="text-sm font-bold text-rp-red">
                      {tier.is_trial ? "Free" : `${(tier.price_paise / 100).toFixed(0)} credits`}
                    </span>
                  </button>
                ))}
              </div>

              <button
                onClick={() => setStep("driver")}
                className="w-full py-2 text-sm text-rp-grey hover:text-white border border-rp-border rounded transition-colors"
              >
                Back
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
