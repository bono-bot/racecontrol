"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { isLoggedIn, api, WalletInfo } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

interface BonusTier {
  id: string;
  min_paise: number;
  bonus_pct: number;
  sort_order: number;
}

const PRESET_AMOUNTS = [500_00, 1000_00, 2000_00, 3000_00, 4000_00, 5000_00];

function getBonusForAmount(
  amountPaise: number,
  tiers: BonusTier[]
): BonusTier | null {
  // Find the highest tier where amount >= min_paise
  const sorted = [...tiers]
    .filter((t) => amountPaise >= t.min_paise)
    .sort((a, b) => b.min_paise - a.min_paise);
  return sorted.length > 0 ? sorted[0] : null;
}

export default function WalletTopUpPage() {
  const router = useRouter();
  const [loading, setLoading] = useState(true);
  const [wallet, setWallet] = useState<WalletInfo | null>(null);
  const [bonusTiers, setBonusTiers] = useState<BonusTier[]>([]);
  const [selectedAmount, setSelectedAmount] = useState<number | null>(null);
  const [showContactStaff, setShowContactStaff] = useState(false);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }

    async function load() {
      try {
        const [walletRes, tiersRes] = await Promise.all([
          api.wallet(),
          api.bonusTiers(),
        ]);
        if (walletRes.wallet) setWallet(walletRes.wallet);
        if (tiersRes.tiers) setBonusTiers(tiersRes.tiers);
      } catch {
        // silent — page still renders
      }
      setLoading(false);
    }

    load();
  }, [router]);

  const handleTopUp = () => {
    if (!selectedAmount) return;
    setShowContactStaff(true);
  };

  const handleBack = () => {
    setShowContactStaff(false);
    setSelectedAmount(null);
  };

  if (loading) {
    return (
      <div className="min-h-screen pb-20">
        <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
          <div className="flex items-center justify-center py-24">
            <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
          </div>
        </div>
        <BottomNav />
      </div>
    );
  }

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        {/* Header */}
        <div className="flex items-center gap-3 mb-6">
          <button
            onClick={() => showContactStaff ? handleBack() : router.back()}
            className="text-rp-grey hover:text-white transition-colors"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              className="h-6 w-6"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M15 19l-7-7 7-7"
              />
            </svg>
          </button>
          <h1 className="text-2xl font-bold text-white">Top Up Wallet</h1>
        </div>

        {/* Current Balance */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-6">
          <p className="text-xs text-rp-grey">Current Balance</p>
          <p className="text-2xl font-bold text-white">
            {wallet ? (wallet.balance_paise / 100).toFixed(0) : "---"}{" "}
            <span className="text-sm font-normal text-rp-grey">Credits</span>
          </p>
        </div>

        {/* Contact Staff State */}
        {showContactStaff ? (
          <div className="bg-rp-card border border-rp-border rounded-xl p-6 text-center">
            {/* Staff icon */}
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-rp-red/20 flex items-center justify-center">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                className="h-8 w-8 text-rp-red"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
                />
              </svg>
            </div>

            <h2 className="text-xl font-bold text-white mb-2">
              Contact Staff at Reception
            </h2>

            <p className="text-sm text-rp-grey mb-6">
              Please visit the reception desk to top up your wallet.
              Our staff will process your payment via Cash or UPI.
            </p>

            {selectedAmount && (
              <div className="bg-black/30 rounded-lg p-4 mb-6">
                <p className="text-xs text-rp-grey mb-1">Amount to Top Up</p>
                <p className="text-2xl font-bold text-white">
                  {(selectedAmount / 100).toFixed(0)} Credits
                </p>
                {(() => {
                  const tier = getBonusForAmount(selectedAmount, bonusTiers);
                  if (!tier) return null;
                  const bonus = Math.floor(
                    (selectedAmount * tier.bonus_pct) / 100
                  );
                  return (
                    <p className="text-sm text-green-400 mt-1">
                      +{(bonus / 100).toFixed(0)} bonus credits ({tier.bonus_pct}%)
                    </p>
                  );
                })()}
              </div>
            )}

            <div className="bg-amber-500/10 border border-amber-500/20 rounded-xl p-4 mb-6">
              <div className="flex items-center gap-2 mb-1">
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  className="h-4 w-4 text-amber-400"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                  />
                </svg>
                <p className="text-sm font-medium text-amber-400">
                  Show this screen to staff
                </p>
              </div>
              <p className="text-xs text-rp-grey">
                Staff will add credits to your wallet after payment is confirmed.
              </p>
            </div>

            <div className="space-y-3">
              <button
                onClick={() => router.push("/wallet/history")}
                className="w-full py-3 text-sm font-medium rounded-xl bg-rp-card border border-rp-border text-white hover:bg-rp-border transition-colors"
              >
                View Transaction History
              </button>
              <button
                onClick={handleBack}
                className="w-full py-3 text-sm font-medium rounded-xl bg-rp-red text-white hover:bg-red-700 transition-colors"
              >
                Change Amount
              </button>
            </div>
          </div>
        ) : (
          <>
            {/* Amount selection */}
            <div className="mb-6">
              <h2 className="text-sm font-medium text-rp-grey mb-3">
                Select Amount
              </h2>
              <div className="grid grid-cols-2 gap-3">
                {PRESET_AMOUNTS.map((amount) => {
                  const tier = getBonusForAmount(amount, bonusTiers);
                  const isSelected = selectedAmount === amount;
                  return (
                    <button
                      key={amount}
                      onClick={() => setSelectedAmount(amount)}
                      className={`relative p-4 rounded-xl border-2 transition-all ${
                        isSelected
                          ? "border-rp-red bg-rp-red/10"
                          : "border-rp-border bg-rp-card hover:border-rp-grey/50"
                      }`}
                    >
                      <p className="text-lg font-bold text-white">
                        {(amount / 100).toFixed(0)}
                      </p>
                      <p className="text-xs text-rp-grey">Credits</p>
                      {tier && (
                        <span className="absolute -top-2 -right-2 bg-green-500 text-white text-[10px] font-bold px-2 py-0.5 rounded-full">
                          +{tier.bonus_pct}%
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Bonus info */}
            {selectedAmount && (() => {
              const tier = getBonusForAmount(selectedAmount, bonusTiers);
              if (!tier) return null;
              const bonus = Math.floor(
                (selectedAmount * tier.bonus_pct) / 100
              );
              return (
                <div className="bg-green-500/10 border border-green-500/20 rounded-xl p-4 mb-6">
                  <div className="flex items-center gap-2 mb-1">
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      className="h-4 w-4 text-green-400"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M12 8v13m0-13V6a2 2 0 112 2h-2zm0 0V5.5A2.5 2.5 0 109.5 8H12zm-7 4h14M5 12a2 2 0 110-4h14a2 2 0 110 4M5 12v7a2 2 0 002 2h10a2 2 0 002-2v-7"
                      />
                    </svg>
                    <p className="text-sm font-medium text-green-400">
                      Bonus: +{tier.bonus_pct}%
                    </p>
                  </div>
                  <p className="text-xs text-rp-grey">
                    You&apos;ll get{" "}
                    <span className="text-white font-medium">
                      {(selectedAmount / 100).toFixed(0)} + {(bonus / 100).toFixed(0)} ={" "}
                      {((selectedAmount + bonus) / 100).toFixed(0)} credits
                    </span>
                  </p>
                </div>
              );
            })()}

            {/* Top Up button */}
            <button
              onClick={handleTopUp}
              disabled={!selectedAmount}
              className={`w-full py-4 text-base font-bold rounded-xl transition-all ${
                selectedAmount
                  ? "bg-rp-red text-white hover:bg-red-700 active:scale-[0.98]"
                  : "bg-rp-card text-rp-grey border border-rp-border cursor-not-allowed"
              }`}
            >
              {selectedAmount ? (
                `Top Up ${(selectedAmount / 100).toFixed(0)} Credits`
              ) : (
                "Select an amount"
              )}
            </button>

            {/* Payment info */}
            <p className="text-center text-xs text-rp-grey mt-4">
              Pay at reception via Cash or UPI
            </p>
          </>
        )}
      </div>

      <BottomNav />
    </div>
  );
}
