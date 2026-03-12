"use client";

import Script from "next/script";
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
  const [processingPayment, setProcessingPayment] = useState(false);
  const [success, setSuccess] = useState(false);
  const [wallet, setWallet] = useState<WalletInfo | null>(null);
  const [bonusTiers, setBonusTiers] = useState<BonusTier[]>([]);
  const [selectedAmount, setSelectedAmount] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [paidAmount, setPaidAmount] = useState(0);
  const [paidBonus, setPaidBonus] = useState(0);

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

  const handleTopUp = async () => {
    if (!selectedAmount || processingPayment) return;

    // Guard: check Razorpay checkout.js is loaded
    if (typeof window === "undefined" || !window.Razorpay) {
      setError("Payment system is still loading. Please wait a moment and try again.");
      return;
    }

    setProcessingPayment(true);
    setError(null);

    try {
      const order = await api.createTopupOrder(selectedAmount);

      if (order.error) {
        setError(order.error);
        setProcessingPayment(false);
        return;
      }

      const tier = getBonusForAmount(selectedAmount, bonusTiers);
      const bonusCredits = tier
        ? Math.floor((selectedAmount * tier.bonus_pct) / 100)
        : 0;

      const options = {
        key: order.key_id,
        amount: order.amount,
        currency: order.currency,
        name: "RacingPoint",
        description: "Wallet Top-up",
        order_id: order.order_id,
        handler: async (response: {
          razorpay_payment_id: string;
          razorpay_order_id: string;
        }) => {
          // Payment succeeded on client side
          // Actual credit happens via webhook — show success UI
          void response;
          setPaidAmount(selectedAmount);
          setPaidBonus(bonusCredits);
          setSuccess(true);
          setProcessingPayment(false);

          // Wait 3 seconds then refresh wallet balance
          setTimeout(async () => {
            try {
              const w = await api.wallet();
              if (w.wallet) setWallet(w.wallet);
            } catch {
              // silent
            }
          }, 3000);
        },
        modal: {
          ondismiss: () => {
            setProcessingPayment(false);
          },
        },
        prefill: {},
        theme: { color: "#E10600" },
      };

      const rzp = new window.Razorpay(options);
      rzp.on("payment.failed", () => {
        setError("Payment failed. Please try again.");
        setProcessingPayment(false);
      });
      rzp.open();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Something went wrong. Please try again."
      );
      setProcessingPayment(false);
    }
  };

  const handleReset = () => {
    setSuccess(false);
    setSelectedAmount(null);
    setPaidAmount(0);
    setPaidBonus(0);
    setError(null);
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
      <Script
        src="https://checkout.razorpay.com/v1/checkout.js"
        strategy="afterInteractive"
      />

      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        {/* Header */}
        <div className="flex items-center gap-3 mb-6">
          <button
            onClick={() => router.back()}
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

        {/* Success State */}
        {success ? (
          <div className="bg-rp-card border border-green-500/30 rounded-xl p-6 text-center">
            {/* Checkmark icon */}
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-green-500/20 flex items-center justify-center">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                className="h-8 w-8 text-green-400"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M5 13l4 4L19 7"
                />
              </svg>
            </div>

            <h2 className="text-xl font-bold text-white mb-2">
              Payment Successful!
            </h2>

            <div className="space-y-2 mb-6">
              <p className="text-sm text-rp-grey">
                <span className="text-white font-medium">
                  +{(paidAmount / 100).toFixed(0)}
                </span>{" "}
                credits added
              </p>
              {paidBonus > 0 && (
                <p className="text-sm text-green-400">
                  +{(paidBonus / 100).toFixed(0)} bonus credits
                </p>
              )}
              <p className="text-xs text-rp-grey mt-2">
                Total:{" "}
                <span className="text-white font-medium">
                  {((paidAmount + paidBonus) / 100).toFixed(0)} credits
                </span>
              </p>
            </div>

            {/* Updated balance */}
            {wallet && (
              <div className="bg-black/30 rounded-lg p-3 mb-6">
                <p className="text-xs text-rp-grey">Updated Balance</p>
                <p className="text-lg font-bold text-white">
                  {(wallet.balance_paise / 100).toFixed(0)} Credits
                </p>
              </div>
            )}

            <div className="space-y-3">
              <button
                onClick={() => router.push("/wallet/history")}
                className="w-full py-3 text-sm font-medium rounded-xl bg-rp-card border border-rp-border text-white hover:bg-rp-border transition-colors"
              >
                View Transaction History
              </button>
              <button
                onClick={handleReset}
                className="w-full py-3 text-sm font-medium rounded-xl bg-rp-red text-white hover:bg-red-700 transition-colors"
              >
                Top Up Again
              </button>
            </div>
          </div>
        ) : (
          <>
            {/* Error */}
            {error && (
              <div className="bg-red-500/10 border border-red-500/30 rounded-xl p-4 mb-4 flex items-start gap-3">
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  className="h-5 w-5 text-red-400 flex-shrink-0 mt-0.5"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                  />
                </svg>
                <div className="flex-1">
                  <p className="text-sm text-red-400">{error}</p>
                </div>
                <button
                  onClick={() => setError(null)}
                  className="text-red-400 hover:text-red-300"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    className="h-4 w-4"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                </button>
              </div>
            )}

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
              disabled={!selectedAmount || processingPayment}
              className={`w-full py-4 text-base font-bold rounded-xl transition-all ${
                selectedAmount && !processingPayment
                  ? "bg-rp-red text-white hover:bg-red-700 active:scale-[0.98]"
                  : "bg-rp-card text-rp-grey border border-rp-border cursor-not-allowed"
              }`}
            >
              {processingPayment ? (
                <span className="flex items-center justify-center gap-2">
                  <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                  Processing...
                </span>
              ) : selectedAmount ? (
                `Top Up ${(selectedAmount / 100).toFixed(0)} Credits`
              ) : (
                "Select an amount"
              )}
            </button>

            {/* Payment methods info */}
            <p className="text-center text-xs text-rp-grey mt-4">
              UPI, Credit Card, Debit Card, Net Banking
            </p>
          </>
        )}
      </div>

      <BottomNav />
    </div>
  );
}
