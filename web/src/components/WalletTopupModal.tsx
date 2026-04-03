"use client";

import { useEffect, useState, useMemo } from "react";
import { api, fetchApi } from "@/lib/api";
import type { Driver } from "@/lib/api";

interface BonusTier {
  min_paise: number;
  bonus_pct: number;
}

type PaymentMethod = "cash" | "upi" | "card";

const QUICK_AMOUNTS = [
  { label: "500", paise: 50000 },
  { label: "700", paise: 70000 },
  { label: "900", paise: 90000 },
  { label: "1000", paise: 100000 },
  { label: "2000", paise: 200000 },
  { label: "3000", paise: 300000 },
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

interface WalletTopupModalProps {
  onClose: () => void;
  onSuccess: () => void;
}

export default function WalletTopupModal({ onClose, onSuccess }: WalletTopupModalProps) {
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [driverSearch, setDriverSearch] = useState("");
  const [selectedDriver, setSelectedDriver] = useState<Driver | null>(null);
  const [balance, setBalance] = useState<number | null>(null);

  const [amount, setAmount] = useState(0);
  const [customAmount, setCustomAmount] = useState("");
  const [method, setMethod] = useState<PaymentMethod>("cash");
  const [bonusTiers, setBonusTiers] = useState<BonusTier[]>([]);

  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

  useEffect(() => {
    api.listDrivers().then((res) => setDrivers(res.drivers || [])).catch(() => {});
    fetchApi<{ tiers?: BonusTier[] }>("/wallet/bonus-tiers")
      .then((res) => { if (res.tiers) setBonusTiers(res.tiers); })
      .catch(() => {});
  }, []);

  const filteredDrivers = useMemo(() => {
    if (!driverSearch.trim()) return [];
    const q = driverSearch.toLowerCase();
    return drivers.filter(
      (d) => d.name.toLowerCase().includes(q) || (d.phone && d.phone.includes(q))
    ).slice(0, 8);
  }, [drivers, driverSearch]);

  async function selectDriver(d: Driver) {
    setSelectedDriver(d);
    setDriverSearch("");
    try {
      const res = await fetchApi<{ wallet?: { balance_paise: number } }>(`/wallet/${d.id}`);
      setBalance(res.wallet?.balance_paise ?? 0);
    } catch {
      setBalance(0);
    }
  }

  const effectiveAmount = amount || (parseInt(customAmount) || 0) * 100;
  const bonus = getBonusForAmount(effectiveAmount, bonusTiers);
  const totalCredits = Math.floor((effectiveAmount + bonus.bonus_paise) / 100);

  async function handleTopup() {
    if (!selectedDriver || effectiveAmount <= 0) return;
    setSubmitting(true);
    setError(null);
    try {
      const res = await fetchApi<{ status?: string; new_balance_paise?: number; error?: string }>(
        `/wallet/${selectedDriver.id}/topup`,
        { method: "POST", body: JSON.stringify({ amount_paise: effectiveAmount, payment_method: method }) }
      );
      if (res.error) {
        setError(res.error);
      } else {
        const newBal = res.new_balance_paise ?? 0;
        setBalance(newBal);
        setSuccessMsg(`Added ${Math.floor(effectiveAmount / 100)} cr — balance is now ${Math.floor(newBal / 100)} cr`);
        setAmount(0);
        setCustomAmount("");
        setTimeout(() => { onSuccess(); onClose(); }, 2000);
      }
    } catch {
      setError("Network error. Try again.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm" onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="bg-rp-card border border-rp-border rounded-xl p-6 w-full max-w-md shadow-2xl max-h-[90vh] overflow-y-auto">
        <h2 className="text-lg font-bold text-white mb-4">Wallet Top-Up</h2>

        {/* Driver search */}
        {!selectedDriver ? (
          <div>
            <label className="block text-xs text-neutral-400 mb-1">Search Customer</label>
            <input
              type="text"
              value={driverSearch}
              onChange={(e) => setDriverSearch(e.target.value)}
              placeholder="Name or phone..."
              className="w-full bg-rp-surface border border-rp-border rounded-lg px-3 py-2 text-sm text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors"
              autoFocus
            />
            {filteredDrivers.length > 0 && (
              <div className="mt-1 border border-rp-border rounded-lg overflow-hidden max-h-48 overflow-y-auto">
                {filteredDrivers.map((d) => (
                  <button
                    key={d.id}
                    onClick={() => selectDriver(d)}
                    className="w-full px-3 py-2 text-left text-sm hover:bg-rp-red/10 transition-colors flex justify-between"
                  >
                    <span className="text-white">{d.name}</span>
                    {d.phone && <span className="text-neutral-500">{d.phone}</span>}
                  </button>
                ))}
              </div>
            )}
            {driverSearch.trim().length > 0 && filteredDrivers.length === 0 && (
              <p className="text-xs text-neutral-500 mt-2">No customers found</p>
            )}
          </div>
        ) : (
          <div>
            {/* Selected driver + balance */}
            <div className="bg-rp-surface border border-rp-border rounded-lg p-3 mb-4 flex justify-between items-center">
              <div>
                <p className="text-white font-medium">{selectedDriver.name}</p>
                <p className="text-xs text-neutral-400">{selectedDriver.phone || "No phone"}</p>
              </div>
              <div className="text-right">
                <p className="text-xs text-neutral-400">Balance</p>
                <p className="text-lg font-bold text-white">{balance !== null ? Math.floor(balance / 100) : "..."} cr</p>
              </div>
            </div>

            {successMsg ? (
              <div className="bg-green-500/10 border border-green-500/30 rounded-lg p-4 text-center">
                <p className="text-green-400 font-medium">{successMsg}</p>
              </div>
            ) : (
              <>
                {/* Quick amounts */}
                <label className="block text-xs text-neutral-400 mb-1">Amount</label>
                <div className="grid grid-cols-3 gap-2 mb-3">
                  {QUICK_AMOUNTS.map((qa) => (
                    <button
                      key={qa.paise}
                      onClick={() => { setAmount(qa.paise); setCustomAmount(""); }}
                      className={`py-2 rounded-lg font-semibold text-sm transition-colors ${
                        amount === qa.paise
                          ? "bg-rp-red text-white"
                          : "bg-rp-surface text-white border border-rp-border hover:border-rp-red/50"
                      }`}
                    >
                      {qa.label}
                    </button>
                  ))}
                </div>
                <input
                  type="number"
                  placeholder="Custom amount (credits)"
                  value={customAmount}
                  onChange={(e) => { setCustomAmount(e.target.value.replace(/\D/g, "")); setAmount(0); }}
                  className="w-full bg-rp-surface border border-rp-border rounded-lg px-3 py-2 text-sm text-white placeholder-zinc-600 focus:outline-none focus:border-rp-red transition-colors mb-3"
                />

                {/* Bonus badge */}
                {bonus.pct > 0 && effectiveAmount > 0 && (
                  <div className="bg-green-500/10 border border-green-500/30 rounded-lg px-3 py-2 mb-3 text-center">
                    <span className="text-green-400 text-sm font-medium">
                      +{bonus.pct}% bonus = {totalCredits} cr total
                    </span>
                  </div>
                )}

                {/* Payment method */}
                <label className="block text-xs text-neutral-400 mb-1">Payment Method</label>
                <div className="flex gap-2 mb-4">
                  {(["cash", "upi", "card"] as PaymentMethod[]).map((m) => (
                    <button
                      key={m}
                      onClick={() => setMethod(m)}
                      className={`flex-1 py-2 rounded-lg text-sm font-medium transition-colors ${
                        method === m
                          ? "bg-rp-red text-white"
                          : "bg-rp-surface text-neutral-400 border border-rp-border hover:text-white"
                      }`}
                    >
                      {m.toUpperCase()}
                    </button>
                  ))}
                </div>

                {error && <p className="text-red-400 text-xs mb-3">{error}</p>}

                {/* Actions */}
                <div className="flex gap-2">
                  <button
                    onClick={() => { setSelectedDriver(null); setBalance(null); setAmount(0); setCustomAmount(""); }}
                    className="flex-1 rounded-lg py-2.5 text-sm font-medium bg-rp-surface text-neutral-400 hover:text-white border border-rp-border transition-colors"
                  >
                    Change Customer
                  </button>
                  <button
                    onClick={handleTopup}
                    disabled={submitting || effectiveAmount <= 0}
                    className="flex-1 rounded-lg py-2.5 text-sm font-semibold bg-rp-red hover:bg-rp-red text-white disabled:opacity-50 transition-colors"
                  >
                    {submitting ? "Adding..." : `Add ${effectiveAmount > 0 ? Math.floor(effectiveAmount / 100) + " cr" : ""}`}
                  </button>
                </div>
              </>
            )}
          </div>
        )}

        {/* Close button */}
        <button
          onClick={onClose}
          className="mt-4 w-full text-center text-neutral-500 text-xs hover:text-white transition-colors"
        >
          Close
        </button>
      </div>
    </div>
  );
}
