"use client";

import { useEffect, useState, useMemo } from "react";
import { api } from "@/lib/api";
import type { Driver, PricingTier } from "@/lib/api";

type PaymentMethod = "wallet" | "cash" | "upi" | "card";

interface BillingStartData {
  pod_id: string;
  driver_id: string;
  pricing_tier_id: string;
  custom_price_paise?: number;
  custom_duration_minutes?: number;
  payment_method: PaymentMethod;
  staff_discount_paise?: number;
  discount_reason?: string;
}

interface BillingAssignData extends BillingStartData {
  auth_type: string;
}

interface BillingStartModalProps {
  podId: string;
  podName: string;
  onClose: () => void;
  onStart: (data: BillingStartData) => void;
  onAssign?: (data: BillingAssignData) => void;
}

type StartMode = "pin" | "qr" | "direct";

const formatCredits = (paise: number) => `${Math.floor(paise / 100)} cr`;

export default function BillingStartModal({
  podId,
  podName,
  onClose,
  onStart,
  onAssign,
}: BillingStartModalProps) {
  const [startMode, setStartMode] = useState<StartMode>("pin");
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [tiers, setTiers] = useState<PricingTier[]>([]);
  const [loading, setLoading] = useState(true);

  const [driverSearch, setDriverSearch] = useState("");
  const [selectedDriver, setSelectedDriver] = useState<Driver | null>(null);
  const [selectedTier, setSelectedTier] = useState<PricingTier | null>(null);

  const [variableTime, setVariableTime] = useState(false);
  const [customMinutes, setCustomMinutes] = useState(30);
  const [customPriceRupees, setCustomPriceRupees] = useState(200);

  const [paymentMethod, setPaymentMethod] = useState<PaymentMethod>("wallet");
  const [showDiscount, setShowDiscount] = useState(false);
  const [discountCredits, setDiscountCredits] = useState(0);
  const [discountReason, setDiscountReason] = useState("");

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
    if (showDiscount && discountCredits > 0 && !discountReason.trim()) return;
    setStarting(true);

    const base: BillingStartData = {
      pod_id: podId,
      driver_id: selectedDriver.id,
      pricing_tier_id: selectedTier?.id || "",
      payment_method: paymentMethod,
    };

    if (variableTime) {
      base.custom_duration_minutes = customMinutes;
      base.custom_price_paise = customPriceRupees * 100;
    }

    if (showDiscount && discountCredits > 0) {
      base.staff_discount_paise = discountCredits * 100;
      base.discount_reason = discountReason.trim();
    }

    if (startMode === "direct") {
      onStart(base);
    } else if (onAssign) {
      onAssign({ ...base, auth_type: startMode });
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-lg bg-rp-card border border-rp-border rounded-xl shadow-2xl p-6 mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h2 className="text-lg font-bold text-white">Start Session</h2>
            <p className="text-sm text-rp-grey">{podName}</p>
          </div>
          <button
            onClick={onClose}
            className="text-rp-grey hover:text-neutral-300 transition-colors text-xl leading-none"
          >
            &times;
          </button>
        </div>

        {loading ? (
          <div className="text-center py-8 text-rp-grey text-sm">
            Loading...
          </div>
        ) : (
          <div className="space-y-6">
            {/* Start Mode */}
            <div>
              <label className="block text-sm font-medium text-neutral-300 mb-2">
                Start Method
              </label>
              <div className="grid grid-cols-3 gap-2">
                {(
                  [
                    { mode: "pin" as StartMode, label: "Assign PIN", desc: "Customer enters PIN" },
                    { mode: "qr" as StartMode, label: "Assign QR", desc: "Customer scans QR" },
                    { mode: "direct" as StartMode, label: "Direct Start", desc: "Staff override" },
                  ] as const
                ).map(({ mode, label, desc }) => (
                  <button
                    key={mode}
                    onClick={() => setStartMode(mode)}
                    className={`rounded-lg border p-2.5 text-left transition-all ${
                      startMode === mode
                        ? "border-rp-red bg-rp-red/10"
                        : "border-rp-border bg-rp-card hover:border-rp-border"
                    }`}
                  >
                    <div className="text-xs font-medium text-neutral-200">
                      {label}
                    </div>
                    <div className="text-[10px] text-rp-grey mt-0.5">
                      {desc}
                    </div>
                  </button>
                ))}
              </div>
            </div>

            {/* Driver Selection */}
            <div>
              <label className="block text-sm font-medium text-neutral-300 mb-2">
                Driver
              </label>
              {selectedDriver ? (
                <div className="flex items-center justify-between bg-rp-card border border-rp-border rounded-lg px-3 py-2">
                  <div className="flex items-center gap-3">
                    <div className="w-8 h-8 rounded-full bg-rp-red/20 flex items-center justify-center text-rp-red font-bold text-sm">
                      {selectedDriver.name.charAt(0).toUpperCase()}
                    </div>
                    <div>
                      <div className="text-sm text-neutral-200">
                        {selectedDriver.name}
                      </div>
                      {selectedDriver.phone && (
                        <div className="text-xs text-rp-grey">
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
                    className="text-xs text-rp-grey hover:text-neutral-300"
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
                    className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-rp-grey focus:outline-none focus:border-rp-red transition-colors"
                    autoFocus
                  />
                  {filteredDrivers.length > 0 && (
                    <div className="mt-1 bg-rp-card border border-rp-border rounded-lg overflow-hidden max-h-48 overflow-y-auto">
                      {filteredDrivers.map((driver) => (
                        <button
                          key={driver.id}
                          onClick={() => setSelectedDriver(driver)}
                          className="w-full flex items-center gap-3 px-3 py-2 hover:bg-rp-card/50 transition-colors text-left"
                        >
                          <div className="w-7 h-7 rounded-full bg-rp-red/20 flex items-center justify-center text-rp-red font-bold text-xs">
                            {driver.name.charAt(0).toUpperCase()}
                          </div>
                          <div>
                            <div className="text-sm text-neutral-200">
                              {driver.name}
                            </div>
                            <div className="text-xs text-rp-grey">
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
              <label className="block text-sm font-medium text-neutral-300 mb-2">
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
                          ? "border-rp-red bg-rp-red/10"
                          : "border-rp-border bg-rp-card hover:border-rp-border"
                      }`}
                    >
                      {tier.is_trial && (
                        <span className="absolute top-2 right-2 bg-emerald-500/20 text-emerald-400 text-[10px] font-bold px-1.5 py-0.5 rounded">
                          FREE
                        </span>
                      )}
                      <div className="text-sm font-medium text-neutral-200">
                        {tier.name}
                      </div>
                      <div className="text-xs text-rp-grey mt-0.5">
                        {tier.duration_minutes} min
                      </div>
                      <div className="text-sm font-bold text-rp-red mt-1">
                        {tier.is_trial
                          ? "Free"
                          : formatCredits(tier.price_paise)}
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
                      ? "border-rp-red bg-rp-red/10"
                      : "border-rp-border bg-rp-card hover:border-rp-border"
                  }`}
                >
                  <div className="text-sm font-medium text-neutral-200">
                    Variable Time
                  </div>
                  <div className="text-xs text-rp-grey mt-0.5">
                    Custom duration
                  </div>
                  <div className="text-sm font-bold text-rp-red mt-1">
                    Custom
                  </div>
                </button>
              </div>
            </div>

            {/* Variable Time inputs */}
            {variableTime && (
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs text-neutral-400 mb-1">
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
                    className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
                  />
                </div>
                <div>
                  <label className="block text-xs text-neutral-400 mb-1">
                    Price (credits)
                  </label>
                  <input
                    type="number"
                    min={0}
                    step={50}
                    value={customPriceRupees}
                    onChange={(e) =>
                      setCustomPriceRupees(parseInt(e.target.value) || 0)
                    }
                    className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
                  />
                </div>
              </div>
            )}

            {/* Payment Method */}
            <div>
              <label className="block text-sm font-medium text-neutral-300 mb-2">
                Payment Method
              </label>
              <div className="grid grid-cols-4 gap-2">
                {(
                  [
                    { value: "wallet" as PaymentMethod, label: "Wallet" },
                    { value: "cash" as PaymentMethod, label: "Cash" },
                    { value: "upi" as PaymentMethod, label: "UPI" },
                    { value: "card" as PaymentMethod, label: "Card" },
                  ] as const
                ).map(({ value, label }) => (
                  <button
                    key={value}
                    onClick={() => setPaymentMethod(value)}
                    className={`rounded-lg border py-2 text-xs font-medium transition-all ${
                      paymentMethod === value
                        ? "border-rp-red bg-rp-red/10 text-neutral-200"
                        : "border-rp-border bg-rp-card text-rp-grey hover:border-rp-border"
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>

            {/* Staff Discount */}
            <div>
              <button
                onClick={() => setShowDiscount(!showDiscount)}
                className="text-xs text-rp-grey hover:text-neutral-300 transition-colors"
              >
                {showDiscount ? "- Hide discount" : "+ Add staff discount"}
              </button>
              {showDiscount && (
                <div className="mt-2 space-y-2">
                  <div>
                    <label className="block text-xs text-neutral-400 mb-1">
                      Discount (credits)
                    </label>
                    <input
                      type="number"
                      min={0}
                      step={10}
                      value={discountCredits}
                      onChange={(e) =>
                        setDiscountCredits(parseInt(e.target.value) || 0)
                      }
                      className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
                    />
                  </div>
                  {discountCredits > 0 && (
                    <div>
                      <label className="block text-xs text-neutral-400 mb-1">
                        Reason (required)
                      </label>
                      <input
                        type="text"
                        placeholder="e.g. loyalty, first-time, event comp"
                        value={discountReason}
                        onChange={(e) => setDiscountReason(e.target.value)}
                        className="w-full bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 placeholder-rp-grey focus:outline-none focus:border-rp-red transition-colors"
                      />
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Start Button */}
            <button
              onClick={handleStart}
              disabled={!canStart || starting || (showDiscount && discountCredits > 0 && !discountReason.trim())}
              className={`w-full rounded-lg py-3 font-semibold text-sm transition-all ${
                canStart && !starting
                  ? "bg-rp-red text-white hover:bg-rp-red active:bg-rp-red"
                  : "bg-rp-card text-rp-grey cursor-not-allowed"
              }`}
            >
              {starting
                ? "Processing..."
                : startMode === "direct"
                ? "Start Session"
                : startMode === "pin"
                ? "Assign with PIN"
                : "Assign with QR"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
