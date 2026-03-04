"use client";

import { useState } from "react";
import { api } from "@/lib/api";

interface WalletTopupProps {
  driverId: string;
  driverName: string;
  currentBalance: number;
  onClose: () => void;
  onSuccess: (newBalance: number) => void;
}

const QUICK_AMOUNTS = [
  { label: "\u20B9500", paise: 50000 },
  { label: "\u20B9700", paise: 70000 },
  { label: "\u20B9900", paise: 90000 },
  { label: "\u20B91000", paise: 100000 },
  { label: "\u20B92000", paise: 200000 },
];

const PAYMENT_METHODS = [
  { id: "cash", label: "Cash" },
  { id: "card", label: "Card" },
  { id: "upi", label: "UPI" },
];

export default function WalletTopup({
  driverId,
  driverName,
  currentBalance,
  onClose,
  onSuccess,
}: WalletTopupProps) {
  const [amount, setAmount] = useState<number>(0);
  const [customAmount, setCustomAmount] = useState("");
  const [method, setMethod] = useState("cash");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const effectiveAmount = amount || (parseInt(customAmount) || 0) * 100;

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
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4">
      <div className="bg-[#1A1A1A] border border-[#333] rounded-2xl w-full max-w-md p-6">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-bold text-white">Top Up Wallet</h2>
          <button onClick={onClose} className="text-[#5A5A5A] text-2xl">
            &times;
          </button>
        </div>

        {/* Driver info */}
        <div className="bg-[#222] rounded-xl p-4 mb-6">
          <p className="text-sm text-[#5A5A5A]">{driverName}</p>
          <p className="text-2xl font-bold text-white">
            {"\u20B9"}{(currentBalance / 100).toFixed(0)}
            <span className="text-sm text-[#5A5A5A] ml-2">current balance</span>
          </p>
        </div>

        {/* Quick amounts */}
        <div className="grid grid-cols-3 gap-2 mb-4">
          {QUICK_AMOUNTS.map((qa) => (
            <button
              key={qa.paise}
              onClick={() => {
                setAmount(qa.paise);
                setCustomAmount("");
              }}
              className={`py-3 rounded-lg font-semibold text-sm transition-colors ${
                amount === qa.paise
                  ? "bg-[#E10600] text-white"
                  : "bg-[#222] text-white border border-[#333]"
              }`}
            >
              {qa.label}
            </button>
          ))}
          <div className="col-span-3">
            <input
              type="number"
              placeholder="Custom amount (\u20B9)"
              value={customAmount}
              onChange={(e) => {
                setCustomAmount(e.target.value);
                setAmount(0);
              }}
              className="w-full bg-[#222] border border-[#333] rounded-lg px-4 py-3 text-white placeholder-[#5A5A5A] text-sm"
            />
          </div>
        </div>

        {/* Payment method */}
        <div className="flex gap-2 mb-6">
          {PAYMENT_METHODS.map((pm) => (
            <button
              key={pm.id}
              onClick={() => setMethod(pm.id)}
              className={`flex-1 py-2 rounded-lg text-sm font-medium transition-colors ${
                method === pm.id
                  ? "bg-[#E10600] text-white"
                  : "bg-[#222] text-[#5A5A5A] border border-[#333]"
              }`}
            >
              {pm.label}
            </button>
          ))}
        </div>

        {error && (
          <p className="text-red-400 text-sm mb-4">{error}</p>
        )}

        {/* Confirm */}
        <button
          onClick={handleTopup}
          disabled={effectiveAmount <= 0 || loading}
          className="w-full bg-[#E10600] text-white font-semibold py-3 rounded-xl disabled:opacity-50"
        >
          {loading
            ? "Processing..."
            : effectiveAmount > 0
            ? `Add \u20B9${(effectiveAmount / 100).toFixed(0)} via ${method.toUpperCase()}`
            : "Select an amount"}
        </button>
      </div>
    </div>
  );
}
