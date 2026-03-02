"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

export default function ScanPage() {
  const router = useRouter();
  const scannerRef = useRef<HTMLDivElement>(null);
  const html5QrRef = useRef<unknown>(null);
  const [scanning, setScanning] = useState(false);
  const [result, setResult] = useState<{
    type: "success" | "error";
    message: string;
  } | null>(null);
  const [driverId, setDriverId] = useState<string | null>(null);

  // Load driver ID on mount
  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    api.profile().then((res) => {
      if (res.driver) setDriverId(res.driver.id);
    });
  }, [router]);

  const handleScan = useCallback(
    async (decodedText: string) => {
      if (!driverId) return;

      // Extract QR token from URL like https://app.racingpoint.in/scan?t=<uuid>
      let qrToken = decodedText;
      try {
        const url = new URL(decodedText);
        const t = url.searchParams.get("t");
        if (t) qrToken = t;
      } catch {
        // Not a URL, use raw text as token
      }

      // Stop scanner
      if (html5QrRef.current) {
        try {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          await (html5QrRef.current as any).stop();
        } catch {
          // ignore
        }
      }
      setScanning(false);

      // Validate QR
      try {
        const res = await api.validateQr(qrToken, driverId);
        if (res.error) {
          setResult({ type: "error", message: res.error });
        } else {
          setResult({
            type: "success",
            message: "Session started! Head to your rig.",
          });
        }
      } catch {
        setResult({ type: "error", message: "Network error. Try again." });
      }
    },
    [driverId]
  );

  const startScanner = useCallback(async () => {
    if (!scannerRef.current) return;

    setResult(null);
    setScanning(true);

    try {
      const { Html5Qrcode } = await import("html5-qrcode");
      const scanner = new Html5Qrcode("qr-reader");
      html5QrRef.current = scanner;

      await scanner.start(
        { facingMode: "environment" },
        {
          fps: 10,
          qrbox: { width: 250, height: 250 },
        },
        (decodedText) => {
          handleScan(decodedText);
        },
        () => {
          // scan error (no QR found in frame) — ignore
        }
      );
    } catch (err) {
      setScanning(false);
      setResult({
        type: "error",
        message: "Camera access denied. Please allow camera permissions.",
      });
    }
  }, [handleScan]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (html5QrRef.current) {
        try {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          (html5QrRef.current as any).stop();
        } catch {
          // ignore
        }
      }
    };
  }, []);

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <h1 className="text-2xl font-bold text-white mb-2">Scan QR</h1>
        <p className="text-rp-grey text-sm mb-6">
          Scan the QR code on your rig screen to start your session
        </p>

        {/* Scanner area */}
        <div className="relative bg-rp-card border border-rp-border rounded-xl overflow-hidden mb-6">
          {scanning ? (
            <div id="qr-reader" ref={scannerRef} className="w-full" />
          ) : (
            <div className="flex flex-col items-center justify-center py-20">
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={1.5}
                className="w-16 h-16 text-rp-grey mb-4"
              >
                <rect x="3" y="3" width="7" height="7" rx="1" />
                <rect x="14" y="3" width="7" height="7" rx="1" />
                <rect x="3" y="14" width="7" height="7" rx="1" />
                <path d="M14 14h3v3M17 20h3v-3M20 14h-3M14 17v3h3" />
              </svg>
              <p className="text-rp-grey text-sm">Camera not active</p>
            </div>
          )}
        </div>

        {/* Result */}
        {result && (
          <div
            className={`rounded-xl p-4 mb-6 ${
              result.type === "success"
                ? "bg-emerald-500/10 border border-emerald-500/30"
                : "bg-red-500/10 border border-red-500/30"
            }`}
          >
            <p
              className={`text-sm font-medium ${
                result.type === "success" ? "text-emerald-400" : "text-red-400"
              }`}
            >
              {result.message}
            </p>
          </div>
        )}

        {/* Scan button */}
        {!scanning && (
          <button
            onClick={startScanner}
            className="w-full bg-rp-red text-white font-semibold py-3.5 rounded-xl active:bg-rp-red-light transition-colors"
          >
            {result ? "Scan Again" : "Open Camera"}
          </button>
        )}

        {scanning && (
          <button
            onClick={async () => {
              if (html5QrRef.current) {
                try {
                  // eslint-disable-next-line @typescript-eslint/no-explicit-any
                  await (html5QrRef.current as any).stop();
                } catch {
                  // ignore
                }
              }
              setScanning(false);
            }}
            className="w-full bg-rp-card text-neutral-300 font-semibold py-3.5 rounded-xl active:bg-rp-card transition-colors"
          >
            Cancel
          </button>
        )}
      </div>
      <BottomNav />
    </div>
  );
}
