"use client";

import { useEffect, useState } from "react";
import { publicApi, getImageBaseUrl } from "@/lib/api";
import type { CafeMenuItem } from "@/lib/api";

function formatPrice(paise: number): string {
  if (paise % 100 === 0) {
    return `Rs. ${paise / 100}`;
  }
  return `Rs. ${(paise / 100).toFixed(2)}`;
}

function CoffeePlaceholder() {
  return (
    <div className="w-full aspect-[4/3] bg-rp-dark flex items-center justify-center">
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.5}
        className="w-10 h-10 text-rp-grey"
      >
        <path d="M17 8h1a4 4 0 010 8h-1" strokeLinecap="round" strokeLinejoin="round" />
        <path
          d="M3 8h14v9a4 4 0 01-4 4H7a4 4 0 01-4-4V8z"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path d="M6 2v3M10 2v3M14 2v3" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

function ItemImage({ item }: { item: CafeMenuItem }) {
  const [imgError, setImgError] = useState(false);
  const imageBaseUrl = getImageBaseUrl();

  if (!item.image_path || imgError) {
    return <CoffeePlaceholder />;
  }

  return (
    <div className="w-full aspect-[4/3] overflow-hidden">
      <img
        src={`${imageBaseUrl}${item.image_path}`}
        alt={item.name}
        className="w-full h-full object-cover"
        onError={() => setImgError(true)}
      />
    </div>
  );
}

function ItemCard({ item }: { item: CafeMenuItem }) {
  return (
    <div className="bg-rp-card rounded-xl overflow-hidden border border-rp-border flex flex-col">
      <ItemImage item={item} />
      <div className="p-3 flex flex-col flex-1">
        <p className="text-sm font-medium text-white line-clamp-2">{item.name}</p>
        {item.description && (
          <p className="text-xs text-rp-grey line-clamp-2 mt-1">{item.description}</p>
        )}
        <p className="text-sm font-bold text-rp-red mt-2">
          {formatPrice(item.selling_price_paise)}
        </p>
      </div>
    </div>
  );
}

function SkeletonCard() {
  return (
    <div className="animate-pulse bg-rp-card rounded-xl h-48 border border-rp-border" />
  );
}

export default function CafePage() {
  const [items, setItems] = useState<CafeMenuItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeCategory, setActiveCategory] = useState<string | null>(null);

  useEffect(() => {
    publicApi
      .cafeMenu()
      .then((res) => {
        setItems(res.items ?? []);
      })
      .catch(() => {
        setItems([]);
      })
      .finally(() => {
        setLoading(false);
      });
  }, []);

  // Collect unique categories in order of appearance
  const categories: string[] = [];
  const seen = new Set<string>();
  for (const item of items) {
    if (!seen.has(item.category_name)) {
      seen.add(item.category_name);
      categories.push(item.category_name);
    }
  }

  // Group items by category_name
  const grouped = new Map<string, CafeMenuItem[]>();
  for (const item of items) {
    const existing = grouped.get(item.category_name);
    if (existing) {
      existing.push(item);
    } else {
      grouped.set(item.category_name, [item]);
    }
  }

  const displayCategories =
    activeCategory !== null ? [activeCategory] : categories;

  return (
    <div className="min-h-screen bg-rp-dark px-4 pt-6">
      <h1 className="text-2xl font-bold text-white mb-4">Cafe Menu</h1>

      {/* Category filter pills */}
      {!loading && categories.length > 0 && (
        <div
          className="flex gap-2 pb-2 overflow-x-auto"
          style={{ msOverflowStyle: "none", scrollbarWidth: "none" } as React.CSSProperties}
        >
          <button
            onClick={() => setActiveCategory(null)}
            className={`flex-shrink-0 px-4 py-1.5 rounded-full text-sm font-medium transition-colors ${
              activeCategory === null
                ? "bg-rp-red text-white"
                : "bg-rp-card text-rp-grey border border-rp-border"
            }`}
          >
            All
          </button>
          {categories.map((cat) => (
            <button
              key={cat}
              onClick={() => setActiveCategory(cat)}
              className={`flex-shrink-0 px-4 py-1.5 rounded-full text-sm font-medium transition-colors ${
                activeCategory === cat
                  ? "bg-rp-red text-white"
                  : "bg-rp-card text-rp-grey border border-rp-border"
              }`}
            >
              {cat}
            </button>
          ))}
        </div>
      )}

      {/* Loading skeletons */}
      {loading && (
        <div className="grid grid-cols-2 gap-3 mt-4">
          {[0, 1, 2, 3, 4, 5].map((i) => (
            <SkeletonCard key={i} />
          ))}
        </div>
      )}

      {/* Empty state */}
      {!loading && items.length === 0 && (
        <div className="flex flex-col items-center justify-center mt-20 gap-4">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={1.5}
            className="w-16 h-16 text-rp-grey"
          >
            <path d="M17 8h1a4 4 0 010 8h-1" strokeLinecap="round" strokeLinejoin="round" />
            <path
              d="M3 8h14v9a4 4 0 01-4 4H7a4 4 0 01-4-4V8z"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path d="M6 2v3M10 2v3M14 2v3" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <p className="text-rp-grey text-center">No items available right now</p>
        </div>
      )}

      {/* Category sections */}
      {!loading &&
        items.length > 0 &&
        displayCategories.map((cat) => {
          const catItems = grouped.get(cat) ?? [];
          if (catItems.length === 0) return null;
          return (
            <div key={cat}>
              {activeCategory === null && (
                <h2 className="text-lg font-semibold text-white mb-3 mt-6">{cat}</h2>
              )}
              <div
                className={`grid grid-cols-2 gap-3 ${activeCategory !== null ? "mt-4" : ""}`}
              >
                {catItems.map((item) => (
                  <ItemCard key={item.id} item={item} />
                ))}
              </div>
            </div>
          );
        })}

      <div className="h-4" />
    </div>
  );
}
