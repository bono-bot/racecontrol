"use client";

import { useState, useEffect } from "react";
import { api } from "@/lib/api";
import type { CafeMenuItem } from "@/lib/types";

function formatPrice(paise: number): string {
  const rupees = paise / 100;
  return rupees % 1 === 0 ? `Rs. ${rupees}` : `Rs. ${rupees.toFixed(2)}`;
}

function groupByCategory(items: CafeMenuItem[]): Map<string, CafeMenuItem[]> {
  const map = new Map<string, CafeMenuItem[]>();
  for (const item of items) {
    const key = item.category_name;
    if (!map.has(key)) map.set(key, []);
    map.get(key)!.push(item);
  }
  return map;
}

export function CafeMenuPanel() {
  const [items, setItems] = useState<CafeMenuItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeCategory, setActiveCategory] = useState<string>("All");

  useEffect(() => {
    api.publicCafeMenu()
      .then((res) => {
        setItems(res.items ?? []);
      })
      .catch((err: unknown) => {
        setError(err instanceof Error ? err.message : "Failed to load menu");
      })
      .finally(() => setLoading(false));
  }, []);

  const grouped = groupByCategory(items);
  const categories = ["All", ...Array.from(grouped.keys())];

  const displayItems: CafeMenuItem[] =
    activeCategory === "All"
      ? items
      : (grouped.get(activeCategory) ?? []);

  if (loading) {
    return (
      <div className="p-4">
        {/* Skeleton shimmer */}
        <div className="flex gap-2 mb-4 overflow-x-auto pb-1">
          {[1, 2, 3].map((i) => (
            <div
              key={i}
              className="h-7 w-20 rounded-full bg-rp-border animate-pulse shrink-0"
            />
          ))}
        </div>
        <div className="grid grid-cols-2 gap-3">
          {[1, 2, 3, 4].map((i) => (
            <div
              key={i}
              className="bg-rp-border rounded-lg p-3 h-16 animate-pulse"
            />
          ))}
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-4 text-center text-rp-grey text-sm">
        {error}
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="p-4 text-center text-rp-grey text-sm">
        No cafe items available
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Category tabs */}
      <div className="flex gap-2 px-4 py-3 overflow-x-auto border-b border-rp-border shrink-0">
        {categories.map((cat) => (
          <button
            key={cat}
            onClick={() => setActiveCategory(cat)}
            className={`px-3 py-1 rounded-full text-xs font-semibold whitespace-nowrap shrink-0 transition-colors ${
              activeCategory === cat
                ? "bg-rp-red text-white"
                : "bg-rp-card border border-rp-border text-rp-grey hover:text-white hover:border-rp-red"
            }`}
          >
            {cat}
          </button>
        ))}
      </div>

      {/* Item grid */}
      <div className="flex-1 overflow-y-auto p-4">
        {activeCategory === "All" ? (
          // When showing all, group with category headers
          Array.from(grouped.entries()).map(([catName, catItems]) => (
            <div key={catName} className="mb-5">
              <h3 className="text-xs font-bold text-rp-grey uppercase tracking-wider mb-2">
                {catName}
              </h3>
              <div className="grid grid-cols-2 gap-3">
                {catItems.map((item) => (
                  <ItemCard key={item.id} item={item} />
                ))}
              </div>
            </div>
          ))
        ) : (
          <div className="grid grid-cols-2 gap-3">
            {displayItems.map((item) => (
              <ItemCard key={item.id} item={item} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

interface ItemCardProps {
  item: CafeMenuItem;
}

function ItemCard({ item }: ItemCardProps) {
  return (
    <div className="bg-rp-dark border border-rp-border rounded-lg p-3 hover:border-rp-red transition-colors cursor-default">
      <p className="font-medium text-white text-sm leading-snug">{item.name}</p>
      <p className="text-rp-red text-xs font-semibold mt-1">
        {formatPrice(item.selling_price_paise)}
      </p>
      {item.description && (
        <p className="text-rp-grey text-[10px] mt-1 line-clamp-2">{item.description}</p>
      )}
    </div>
  );
}
