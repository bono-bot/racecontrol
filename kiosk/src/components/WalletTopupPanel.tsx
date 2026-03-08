"use client";

import { useState, useEffect } from "react";
import { api, fetchApi } from "@/lib/api";

interface WalletTopupPanelProps {
  driverId: string;
  driverName: string;
  currentBalance: number;
  onClose: () => void;
  onSuccess: (newBalance: number) => void;
}

interface BonusTier {
  min_paise: number;
  bonus_pct: number;
}

const QUICK_AMOUNTS = [
  { label: "500", paise: 50000 },
  { label: "700", paise: 70000 },
  { label: "900", paise: 90000 },
  { label: "1000", paise: 100000 },
  { label: "2000", paise: 200000 },
  { label: "3000", paise: 300000 },
  { label: "4000", paise: 400000 },
];

const PAYMENT_METHODS = [
  { id: "cash", label: "Cash" },
  { id: "card", label: "Card" },
  { id: "upi", label: "UPI" },
];

function getBonusForAmount(paise: number, tiers: BonusTier[]): { pct: number; bonus_paise: number } {
  const sorted = [...tiers].sort((a, b) => b.min_paise - a.min_paise);
  for (const tier of sorted) {
    if (paise >= tier.min_paise) {
      return { pct: tier.bonus_pct, bonus_paise: (paise * tier.bonus_pct) / 100 };
    }
  }
  return { pct: 0, bonus_paise: 0 };
}

export function WalletTopupPanel({
  driverId,
  driverName,
  currentBalance,
  onClose,
  onSuccess,
}: WalletTopupPanelProps) {
  const [amount, setAmount] = useState<number>(0);
  const [customAmount, setCustomAmount] = useState("");
  const [method, setMethod] = useState("cash");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bonusTiers, setBonusTiers] = useState<BonusTier[]>([]);

  const effectiveAmount = amount || (parseInt(customAmount) || 0) * 100;
  const bonus = getBonusForAmount(effectiveAmount, bonusTiers);
  const totalCredits = (effectiveAmount + bonus.bonus_paise) / 100;

  useEffect(() => {
    fetchApi<{ tiers?: BonusTier[] }>("/wallet/bonus-tiers")
      .then((res) => {
        if (res.tiers) setBonusTiers(res.tiers);
      })
      .catch(() => {});
  }, []);

  async function handleTopup() {
    if (effectiveAmount <= 0) return;
    setLoading(true);
    setError(null);

    try {
      const res = await api.topupWallet(driverId, effectiveAmount, method);
      if (res.new_balance_paise !== undefined) {
        onSuccess(res.new_balance_paise);
      } else {
        setError(res.error || "Top-up failed");
      }
    } catch {
      setError("Network error");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="flex flex-col h-full p-5">
      {/* Driver info */}
      <div className="bg-rp-surface border border-rp-border rounded-xl p-4 mb-5">
        <p className="text-sm text-rp-grey">{driverName}</p>
        <p className="text-2xl font-bold text-white">
          {(currentBalance / 100).toFixed(0)} credits
          <span className="text-sm text-rp-grey ml-2">current balance</span>
        </p>
      </div>

      {/* Quick amounts */}
      <div className="grid grid-cols-4 gap-2 mb-4">
        {QUICK_AMOUNTS.map((qa) => (
          <button
            key={qa.paise}
            onClick={() => {
              setAmount(qa.paise);
              setCustomAmount("");
            }}
            className={`py-2.5 rounded-lg font-semibold text-sm transition-colors ${
              amount === qa.paise
                ? "bg-rp-red text-white"
                : "bg-rp-surface text-white border border-rp-border"
            }`}
          >
            {qa.label}
          </button>
        ))}
        <div className="col-span-4">
          <input
            type="number"
            placeholder="Custom amount (credits)"
            value={customAmount}
            onChange={(e) => {
              setCustomAmount(e.target.value);
              setAmount(0);
            }}
            className="w-full bg-rp-surface border border-rp-border rounded-lg px-4 py-2.5 text-white placeholder-rp-grey text-sm"
          />
        </div>
      </div>

      {/* Bonus badge */}
      {bonus.pct > 0 && effectiveAmount > 0 && (
        <div className="bg-green-900/30 border border-green-600/40 rounded-lg px-4 py-2 mb-4 text-center">
          <span className="text-green-400 font-semibold text-sm">
            +{bonus.pct}% bonus = {(bonus.bonus_paise / 100).toFixed(0)} extra credits
          </span>
        </div>
      )}

      {/* Payment method */}
      <div className="flex gap-2 mb-5">
        {PAYMENT_METHODS.map((pm) => (
          <button
            key={pm.id}
            onClick={() => setMethod(pm.id)}
            className={`flex-1 py-2 rounded-lg text-sm font-medium transition-colors ${
              method === pm.id
                ? "bg-rp-red text-white"
                : "bg-rp-surface text-rp-grey border border-rp-border"
            }`}
          >
            {pm.label}
          </button>
        ))}
      </div>

      {error && <p className="text-red-400 text-sm mb-4">{error}</p>}

      {/* Spacer */}
      <div className="flex-1" />

      {/* Confirm */}
      <button
        onClick={handleTopup}
        disabled={effectiveAmount <= 0 || loading}
        className="w-full bg-rp-red text-white font-semibold py-3 rounded-xl disabled:opacity-50 transition-colors"
      >
        {loading
          ? "Processing..."
          : effectiveAmount > 0
          ? bonus.pct > 0
            ? `Add ${totalCredits.toFixed(0)} credits (incl. ${(bonus.bonus_paise / 100).toFixed(0)} bonus) via ${method.toUpperCase()}`
            : `Add ${(effectiveAmount / 100).toFixed(0)} credits via ${method.toUpperCase()}`
          : "Select an amount"}
      </button>

      <button
        onClick={onClose}
        className="w-full py-2 mt-2 text-sm text-rp-grey hover:text-white transition-colors"
      >
        Back to Session
      </button>
    </div>
  );
}
