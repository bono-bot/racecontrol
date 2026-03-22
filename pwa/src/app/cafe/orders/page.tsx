"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api, CafeOrderHistoryItem } from "@/lib/api";

function formatPrice(paise: number): string {
  if (paise % 100 === 0) return `Rs. ${paise / 100}`;
  return `Rs. ${(paise / 100).toFixed(2)}`;
}

function formatOrderDate(dateStr: string): string {
  // Parse "2026-03-22 14:35:00" as UTC, display as IST
  const d = new Date(dateStr.replace(" ", "T") + "Z");
  return d.toLocaleString("en-IN", {
    timeZone: "Asia/Kolkata",
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: true,
  });
}

export default function CafeOrdersPage() {
  const [orders, setOrders] = useState<CafeOrderHistoryItem[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  useEffect(() => {
    api
      .getCafeOrderHistory()
      .then((res) => {
        setOrders(res.orders ?? []);
      })
      .catch(() => {
        setOrders([]);
      })
      .finally(() => {
        setLoading(false);
      });
  }, []);

  return (
    <div className="min-h-screen bg-rp-dark px-4 py-6">
      <h1 className="text-white font-bold text-2xl mb-6">Order History</h1>

      {loading && (
        <ul className="space-y-3">
          {[0, 1, 2].map((i) => (
            <li
              key={i}
              className="animate-pulse bg-rp-card h-16 rounded-xl"
            />
          ))}
        </ul>
      )}

      {!loading && orders.length === 0 && (
        <div className="flex flex-col items-center justify-center mt-20 gap-4">
          <p className="text-rp-grey text-center text-sm">
            No cafe orders yet. Head to the menu to place your first order.
          </p>
          <Link
            href="/cafe"
            className="bg-rp-red text-white font-semibold px-6 py-3 rounded-xl text-sm"
          >
            Go to Menu
          </Link>
        </div>
      )}

      {!loading && orders.length > 0 && (
        <ul>
          {orders.map((order) => (
            <li
              key={order.id}
              className="border-b border-rp-border"
            >
              <button
                type="button"
                className="w-full text-left py-4 flex items-center gap-3"
                onClick={() =>
                  setExpandedId((prev) =>
                    prev === order.id ? null : order.id
                  )
                }
              >
                <div className="flex-1 min-w-0">
                  <p className="text-white font-medium truncate">
                    {order.receipt_number}
                  </p>
                  <p className="text-rp-grey text-xs mt-0.5">
                    {formatOrderDate(order.created_at)}
                  </p>
                  <p className="text-rp-grey text-xs mt-0.5">
                    {order.items.length === 1
                      ? "1 item"
                      : `${order.items.length} items`}
                  </p>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <span className="text-white font-bold text-sm">
                    {formatPrice(order.total_paise)}
                  </span>
                  <svg
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={2}
                    className={`w-4 h-4 text-rp-grey transition-transform ${
                      expandedId === order.id ? "rotate-180" : ""
                    }`}
                  >
                    <path
                      d="M19 9l-7 7-7-7"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                </div>
              </button>

              {expandedId === order.id && (
                <ul className="pb-4 space-y-2">
                  {order.items.map((item) => (
                    <li
                      key={item.item_id}
                      className="flex justify-between items-center text-sm"
                    >
                      <span className="text-rp-grey">
                        {item.name} x{item.quantity}
                      </span>
                      <span className="text-white">
                        {formatPrice(item.line_total_paise)}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
