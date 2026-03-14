"use client";

import { useEffect, useState, useRef } from "react";
import Link from "next/link";
import { publicApi } from "@/lib/api";

interface DriverResult {
  id: string;
  display_name: string;
  total_laps: number;
  avatar_url: string | null;
}

function getInitials(name: string): string {
  return name
    .split(" ")
    .map((w) => w[0])
    .filter(Boolean)
    .slice(0, 2)
    .join("")
    .toUpperCase();
}

export default function DriversPage() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<DriverResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [searched, setSearched] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (timerRef.current) clearTimeout(timerRef.current);

    if (query.length < 2) {
      setResults([]);
      setSearched(false);
      return;
    }

    setLoading(true);
    timerRef.current = setTimeout(() => {
      publicApi
        .searchDrivers(query)
        .then((data: { drivers?: DriverResult[] }) => {
          setResults(data.drivers || []);
          setSearched(true);
          setLoading(false);
        })
        .catch(() => {
          setResults([]);
          setSearched(true);
          setLoading(false);
        });
    }, 300);

    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [query]);

  return (
    <div className="min-h-screen bg-rp-dark">
      {/* Header */}
      <div className="bg-gradient-to-b from-rp-red/20 to-transparent pt-12 pb-8 px-4">
        <div className="max-w-2xl mx-auto text-center">
          <h1 className="text-3xl font-bold text-white tracking-tight">
            Drivers
          </h1>
          <p className="text-rp-grey text-sm mt-1">
            Find a driver and view their profile
          </p>
        </div>
      </div>

      <div className="max-w-2xl mx-auto px-4 pb-8">
        {/* Search input */}
        <div className="relative mb-6">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-rp-grey"
          >
            <circle cx="11" cy="11" r="8" />
            <path d="M21 21l-4.35-4.35" strokeLinecap="round" />
          </svg>
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search by name..."
            className="w-full bg-rp-card border border-rp-border rounded-xl pl-10 pr-4 py-3 text-sm text-white placeholder-rp-grey focus:border-rp-red focus:outline-none"
          />
        </div>

        {/* Loading state */}
        {loading && (
          <div className="flex justify-center py-12">
            <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
          </div>
        )}

        {/* Empty state - not searched yet */}
        {!loading && !searched && (
          <div className="text-center py-12">
            <p className="text-rp-grey text-sm">
              Search for a driver by name
            </p>
          </div>
        )}

        {/* No results */}
        {!loading && searched && results.length === 0 && (
          <div className="text-center py-12">
            <p className="text-rp-grey text-sm">
              No drivers found matching &apos;{query}&apos;
            </p>
          </div>
        )}

        {/* Results grid */}
        {!loading && results.length > 0 && (
          <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
            {results.map((driver) => (
              <Link
                key={driver.id}
                href={`/drivers/${driver.id}`}
                className="bg-rp-card border border-rp-border rounded-xl p-4 hover:border-rp-red/30 transition-colors text-center"
              >
                {driver.avatar_url ? (
                  <img
                    src={driver.avatar_url}
                    alt={driver.display_name}
                    className="w-14 h-14 rounded-full mx-auto mb-2 object-cover"
                  />
                ) : (
                  <div className="w-14 h-14 rounded-full mx-auto mb-2 bg-rp-red/20 flex items-center justify-center text-rp-red font-bold text-lg">
                    {getInitials(driver.display_name)}
                  </div>
                )}
                <p className="text-sm text-white font-medium truncate">
                  {driver.display_name}
                </p>
                <p className="text-xs text-rp-grey mt-0.5">
                  {driver.total_laps} laps
                </p>
              </Link>
            ))}
          </div>
        )}

        {/* Footer */}
        <div className="text-center mt-8">
          <p className="text-rp-grey text-xs">RacingPoint</p>
        </div>
      </div>
    </div>
  );
}
