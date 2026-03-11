"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { isLoggedIn, getToken } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

const API_BASE =
  process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api/v1";

interface Transaction {
  id: string;
  driver_id: string;
  amount_paise: number;
  balance_after_paise: number;
  txn_type: string;
  reference_id: string | null;
  notes: string | null;
  staff_id: string | null;
  created_at: string;
}

const TXN_LABELS: Record<string, string> = {
  topup_cash: "Top-up (Cash)",
  topup_card: "Top-up (Card)",
  topup_upi: "Top-up (UPI)",
  topup_online: "Top-up (Online)",
  debit_session: "Racing Session",
  debit_cafe: "Cafe",
  debit_merchandise: "Merchandise",
  debit_penalty: "Penalty",
  refund_session: "Session Refund",
  refund_manual: "Refund",
  bonus: "Bonus",
  adjustment: "Adjustment",
};

function isCredit(txnType: string): boolean {
  return (
    txnType.startsWith("topup") ||
    txnType === "bonus" ||
    txnType.startsWith("refund") ||
    txnType === "adjustment"
  );
}

function formatDate(dateStr: string): string {
  try {
    const d = new Date(dateStr + "Z");
    return d.toLocaleDateString("en-IN", {
      day: "numeric",
      month: "short",
      year: "numeric",
    });
  } catch {
    return dateStr;
  }
}

function formatTime(dateStr: string): string {
  try {
    const d = new Date(dateStr + "Z");
    return d.toLocaleTimeString("en-IN", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: true,
    });
  } catch {
    return "";
  }
}

export default function WalletHistoryPage() {
  const router = useRouter();
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [page, setPage] = useState(0);
  const PAGE_SIZE = 50;

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    fetchTransactions(0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [router]);

  const fetchTransactions = async (pageNum: number) => {
    setLoading(true);
    try {
      const token = getToken();
      const res = await fetch(
        `${API_BASE}/customer/wallet/transactions?limit=${PAGE_SIZE}&offset=${pageNum * PAGE_SIZE}`,
        {
          headers: {
            Authorization: `Bearer ${token}`,
            "Content-Type": "application/json",
          },
        }
      );
      const data = await res.json();
      if (data.transactions) {
        setTransactions(data.transactions);
        setTotal(data.total || data.transactions.length);
        setPage(pageNum);
      }
    } catch {
      // silent
    }
    setLoading(false);
  };

  const totalPages = Math.ceil(total / PAGE_SIZE);

  return (
    <div className="min-h-screen pb-20">
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
          <h1 className="text-2xl font-bold text-white">Transaction History</h1>
        </div>

        {/* Summary */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-6">
          <p className="text-xs text-rp-grey">Total Transactions</p>
          <p className="text-xl font-bold text-white">{total}</p>
        </div>

        {/* Transaction list */}
        {loading ? (
          <div className="flex items-center justify-center py-12">
            <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
          </div>
        ) : transactions.length === 0 ? (
          <div className="text-center py-12">
            <p className="text-rp-grey text-sm">No transactions yet</p>
          </div>
        ) : (
          <div className="space-y-2">
            {transactions.map((txn) => {
              const credit = isCredit(txn.txn_type);
              const amount = Math.abs(txn.amount_paise);
              return (
                <div
                  key={txn.id}
                  className="bg-rp-card border border-rp-border rounded-xl p-4"
                >
                  <div className="flex justify-between items-start mb-1">
                    <div>
                      <p className="text-sm font-medium text-white">
                        {TXN_LABELS[txn.txn_type] || txn.txn_type}
                      </p>
                      <p className="text-xs text-rp-grey">
                        {formatDate(txn.created_at)} at{" "}
                        {formatTime(txn.created_at)}
                      </p>
                    </div>
                    <div className="text-right">
                      <p
                        className={`text-sm font-bold ${
                          credit ? "text-green-400" : "text-red-400"
                        }`}
                      >
                        {credit ? "+" : "-"}
                        {(amount / 100).toFixed(0)} credits
                      </p>
                      <p className="text-xs text-rp-grey">
                        Bal: {(txn.balance_after_paise / 100).toFixed(0)}
                      </p>
                    </div>
                  </div>
                  {txn.notes && (
                    <p className="text-xs text-rp-grey mt-1">{txn.notes}</p>
                  )}
                </div>
              );
            })}
          </div>
        )}

        {/* Pagination */}
        {totalPages > 1 && (
          <div className="flex items-center justify-between mt-6">
            <button
              onClick={() => fetchTransactions(page - 1)}
              disabled={page === 0}
              className="px-4 py-2 text-sm font-medium rounded-lg bg-rp-card border border-rp-border text-white disabled:opacity-30 disabled:cursor-not-allowed"
            >
              Previous
            </button>
            <span className="text-xs text-rp-grey">
              Page {page + 1} of {totalPages}
            </span>
            <button
              onClick={() => fetchTransactions(page + 1)}
              disabled={page >= totalPages - 1}
              className="px-4 py-2 text-sm font-medium rounded-lg bg-rp-card border border-rp-border text-white disabled:opacity-30 disabled:cursor-not-allowed"
            >
              Next
            </button>
          </div>
        )}
      </div>
      <BottomNav />
    </div>
  );
}
