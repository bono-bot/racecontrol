"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import { api } from "@/lib/api";
import type { Driver, KioskPinResponse } from "@/lib/api";

type KioskView = "pin" | "lookup" | "success";

export default function KioskPage() {
  const [view, setView] = useState<KioskView>("pin");

  // PIN entry state
  const [pin, setPin] = useState("");
  const [pinError, setPinError] = useState<string | null>(null);
  const [pinLoading, setPinLoading] = useState(false);

  // Success state
  const [successData, setSuccessData] = useState<KioskPinResponse | null>(null);

  // Lookup state
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<Driver[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);

  const resetToPin = useCallback(() => {
    setView("pin");
    setPin("");
    setPinError(null);
    setSuccessData(null);
  }, []);

  // Auto-reset from success screen after 10s
  useEffect(() => {
    if (view === "success") {
      const timer = setTimeout(resetToPin, 10000);
      return () => clearTimeout(timer);
    }
  }, [view, resetToPin]);

  // === PIN Logic ===

  const submitPin = useCallback(async (pinValue: string) => {
    setPinLoading(true);
    setPinError(null);
    try {
      const res = await api.kioskValidatePin(pinValue);
      if (res.error) {
        setPinError(res.error);
        setPin("");
      } else {
        setSuccessData(res);
        setView("success");
      }
    } catch {
      setPinError("Connection error. Please try again.");
      setPin("");
    } finally {
      setPinLoading(false);
    }
  }, []);

  const handlePinDigit = useCallback(
    (digit: string) => {
      if (pinLoading) return;
      setPin((prev) => {
        if (prev.length >= 4) return prev;
        const newPin = prev + digit;
        setPinError(null);
        if (newPin.length === 4) {
          submitPin(newPin);
        }
        return newPin;
      });
    },
    [pinLoading, submitPin]
  );

  const handlePinBackspace = useCallback(() => {
    setPin((prev) => prev.slice(0, -1));
    setPinError(null);
  }, []);

  const handlePinClear = useCallback(() => {
    setPin("");
    setPinError(null);
  }, []);

  // Keyboard support for PIN entry
  useEffect(() => {
    if (view !== "pin") return;
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key >= "0" && e.key <= "9") {
        handlePinDigit(e.key);
      } else if (e.key === "Backspace") {
        handlePinBackspace();
      } else if (e.key === "Escape") {
        handlePinClear();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [view, handlePinDigit, handlePinBackspace, handlePinClear]);

  // === Lookup Logic ===

  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (view !== "lookup") return;
    if (!searchQuery.trim()) {
      setSearchResults([]);
      return;
    }
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(async () => {
      setSearchLoading(true);
      try {
        const res = await api.listDrivers(searchQuery.trim());
        setSearchResults(res.drivers || []);
      } catch {
        setSearchResults([]);
      } finally {
        setSearchLoading(false);
      }
    }, 300);
    return () => {
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    };
  }, [searchQuery, view]);

  return (
    <div className="min-h-screen bg-rp-black flex flex-col">
      {/* Header */}
      <header className="flex items-center justify-between px-8 py-4 border-b border-rp-border">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold text-rp-red tracking-tight">
            RACINGPOINT
          </h1>
          <span className="text-rp-grey text-sm">Kiosk Terminal</span>
        </div>
        {view !== "pin" && (
          <button
            onClick={resetToPin}
            className="text-sm text-rp-grey hover:text-white transition-colors"
          >
            &larr; Back to PIN Entry
          </button>
        )}
      </header>

      {/* Main */}
      <main className="flex-1 flex items-center justify-center p-8">
        {view === "pin" && (
          <PinEntryPanel
            pin={pin}
            error={pinError}
            loading={pinLoading}
            onDigit={handlePinDigit}
            onBackspace={handlePinBackspace}
            onClear={handlePinClear}
            onSwitchToLookup={() => {
              setView("lookup");
              setSearchQuery("");
              setSearchResults([]);
            }}
          />
        )}

        {view === "lookup" && (
          <CustomerLookupPanel
            query={searchQuery}
            onQueryChange={setSearchQuery}
            results={searchResults}
            loading={searchLoading}
          />
        )}

        {view === "success" && successData && (
          <SuccessPanel data={successData} onDone={resetToPin} />
        )}
      </main>

      {/* Footer */}
      <footer className="px-8 py-3 border-t border-rp-border text-center">
        <p className="text-xs text-rp-grey">May the Fastest Win.</p>
      </footer>
    </div>
  );
}

// ─── PIN Entry ────────────────────────────────────────────────────────────────

function PinEntryPanel({
  pin,
  error,
  loading,
  onDigit,
  onBackspace,
  onClear,
  onSwitchToLookup,
}: {
  pin: string;
  error: string | null;
  loading: boolean;
  onDigit: (d: string) => void;
  onBackspace: () => void;
  onClear: () => void;
  onSwitchToLookup: () => void;
}) {
  return (
    <div className="w-full max-w-sm text-center">
      <h2 className="text-2xl font-bold text-white mb-2">Enter Your PIN</h2>
      <p className="text-rp-grey text-sm mb-8">
        Enter the 4-digit PIN provided at reception
      </p>

      {/* PIN display */}
      <div className="flex justify-center gap-4 mb-8">
        {[0, 1, 2, 3].map((i) => (
          <div
            key={i}
            className={`w-14 h-14 rounded-xl border-2 flex items-center justify-center text-2xl font-bold transition-all ${
              i < pin.length
                ? "border-rp-red bg-rp-red/10 text-white"
                : "border-rp-border bg-rp-card text-transparent"
            }`}
          >
            {i < pin.length ? pin[i] : "0"}
          </div>
        ))}
      </div>

      {/* Error */}
      {error && (
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg px-4 py-3 mb-6">
          <p className="text-red-400 text-sm">{error}</p>
        </div>
      )}

      {/* Loading */}
      {loading && (
        <div className="mb-6">
          <p className="text-rp-grey text-sm animate-pulse">
            Verifying PIN...
          </p>
        </div>
      )}

      {/* Numpad */}
      <div className="grid grid-cols-3 gap-3 max-w-[280px] mx-auto mb-8">
        {["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((d) => (
          <button
            key={d}
            onClick={() => onDigit(d)}
            disabled={loading || pin.length >= 4}
            className="h-16 rounded-xl bg-rp-card border border-rp-border text-white text-xl font-semibold
                       hover:bg-rp-border hover:border-rp-grey active:bg-rp-red/20
                       disabled:opacity-30 disabled:cursor-not-allowed transition-all"
          >
            {d}
          </button>
        ))}
        <button
          onClick={onClear}
          disabled={loading}
          className="h-16 rounded-xl bg-rp-card border border-rp-border text-rp-grey text-sm font-medium
                     hover:bg-rp-border transition-all disabled:opacity-30"
        >
          Clear
        </button>
        <button
          onClick={() => onDigit("0")}
          disabled={loading || pin.length >= 4}
          className="h-16 rounded-xl bg-rp-card border border-rp-border text-white text-xl font-semibold
                     hover:bg-rp-border hover:border-rp-grey active:bg-rp-red/20
                     disabled:opacity-30 disabled:cursor-not-allowed transition-all"
        >
          0
        </button>
        <button
          onClick={onBackspace}
          disabled={loading || pin.length === 0}
          className="h-16 rounded-xl bg-rp-card border border-rp-border text-rp-grey text-lg font-medium
                     hover:bg-rp-border transition-all disabled:opacity-30"
        >
          &#9003;
        </button>
      </div>

      {/* Switch to lookup */}
      <button
        onClick={onSwitchToLookup}
        className="text-lg text-rp-grey hover:text-rp-red transition-colors py-3 px-6 border border-rp-border rounded-xl hover:border-rp-grey"
      >
        Don&apos;t have a PIN? Search your account
      </button>
    </div>
  );
}

// ─── Customer Lookup ──────────────────────────────────────────────────────────

function CustomerLookupPanel({
  query,
  onQueryChange,
  results,
  loading,
}: {
  query: string;
  onQueryChange: (q: string) => void;
  results: Driver[];
  loading: boolean;
}) {
  return (
    <div className="w-full max-w-lg">
      <h2 className="text-2xl font-bold text-white mb-2 text-center">
        Customer Lookup
      </h2>
      <p className="text-rp-grey text-sm mb-6 text-center">
        Search by name or phone number
      </p>

      {/* Search input */}
      <div className="relative mb-6">
        <input
          type="text"
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
          placeholder="Type a name or phone number..."
          autoFocus
          className="w-full bg-rp-card border border-rp-border rounded-xl px-4 py-3.5 text-white text-lg
                     placeholder:text-rp-grey
                     focus:outline-none focus:border-rp-red transition-colors"
        />
        {loading && (
          <div className="absolute right-4 top-1/2 -translate-y-1/2">
            <div className="w-5 h-5 border-2 border-rp-grey border-t-rp-red rounded-full animate-spin" />
          </div>
        )}
      </div>

      {/* Results */}
      {results.length > 0 ? (
        <div className="space-y-2 max-h-[400px] overflow-y-auto">
          {results.map((driver) => (
            <div
              key={driver.id}
              className="bg-rp-card border border-rp-border rounded-xl px-4 py-3 flex items-center gap-4"
            >
              <div className="w-10 h-10 rounded-full bg-rp-red/20 flex items-center justify-center text-rp-red font-bold shrink-0">
                {driver.name.charAt(0).toUpperCase()}
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-white font-medium truncate">
                  {driver.name}
                </div>
                <div className="text-rp-grey text-sm">
                  {driver.phone || "No phone"}
                  {driver.customer_id && (
                    <span className="ml-2 text-rp-red">
                      {driver.customer_id}
                    </span>
                  )}
                </div>
              </div>
              <div className="text-xs text-rp-grey shrink-0">
                {driver.total_laps} laps
              </div>
            </div>
          ))}
        </div>
      ) : query.trim() && !loading ? (
        <div className="text-center py-8">
          <p className="text-rp-grey text-sm">
            No customers found for &quot;{query}&quot;
          </p>
          <p className="text-rp-grey text-xs mt-2">
            Customer must register at app.racingpoint.cloud first
          </p>
        </div>
      ) : !query.trim() ? (
        <div className="text-center py-8">
          <p className="text-rp-grey text-sm">
            Start typing to search registered customers
          </p>
        </div>
      ) : null}
    </div>
  );
}

// ─── Success ──────────────────────────────────────────────────────────────────

function SuccessPanel({
  data,
  onDone,
}: {
  data: KioskPinResponse;
  onDone: () => void;
}) {
  const minutes = data.allocated_seconds
    ? Math.floor(data.allocated_seconds / 60)
    : 0;

  return (
    <div className="w-full max-w-md text-center">
      {/* Checkmark */}
      <div className="w-20 h-20 rounded-full bg-emerald-500/20 border-2 border-emerald-500 flex items-center justify-center mx-auto mb-6">
        <svg
          className="w-10 h-10 text-emerald-400"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={3}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M5 13l4 4L19 7"
          />
        </svg>
      </div>

      <h2 className="text-3xl font-bold text-white mb-2">Session Started!</h2>

      <div className="bg-rp-card border border-rp-border rounded-xl p-6 mt-6 space-y-4">
        <div className="flex justify-between items-center">
          <span className="text-rp-grey text-sm">Driver</span>
          <span className="text-white font-semibold">{data.driver_name}</span>
        </div>
        <div className="border-t border-rp-border" />
        <div className="flex justify-between items-center">
          <span className="text-rp-grey text-sm">Pod</span>
          <span className="text-rp-red font-bold text-lg">
            Pod {data.pod_number}
          </span>
        </div>
        <div className="border-t border-rp-border" />
        <div className="flex justify-between items-center">
          <span className="text-rp-grey text-sm">Session</span>
          <span className="text-white font-semibold">
            {data.pricing_tier_name}
          </span>
        </div>
        {minutes > 0 && (
          <>
            <div className="border-t border-rp-border" />
            <div className="flex justify-between items-center">
              <span className="text-rp-grey text-sm">Duration</span>
              <span className="text-white font-semibold">{minutes} min</span>
            </div>
          </>
        )}
      </div>

      <p className="text-rp-grey text-sm mt-6 mb-4">
        Head to <span className="text-white font-semibold">Pod {data.pod_number}</span> and start racing!
      </p>

      <button
        onClick={onDone}
        className="px-8 py-3 bg-rp-card border border-rp-border text-white rounded-xl
                   hover:bg-rp-border transition-colors"
      >
        Done
      </button>

      <p className="text-xs text-rp-grey mt-4 animate-pulse">
        Returning to PIN entry in a few seconds...
      </p>
    </div>
  );
}
