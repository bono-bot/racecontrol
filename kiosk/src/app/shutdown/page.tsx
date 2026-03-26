"use client";

import { useState, useEffect } from "react";
import { StaffLoginScreen } from "@/components/StaffLoginScreen";
import { api } from "@/lib/api";
import type { VenueShutdownResponse } from "@/lib/types";

type ShutdownState =
  | "idle"
  | "confirming"
  | "auditing"
  | "shutting_down"
  | "complete"
  | "audit_blocked"
  | "error";

interface DeviceStatus {
  label: string;
  status: "waiting" | "in_progress" | "done";
}

const INITIAL_DEVICES: DeviceStatus[] = [
  { label: "Pods 1–8", status: "waiting" },
  { label: "POS PC", status: "waiting" },
  { label: "Server (.23)", status: "waiting" },
];

export default function ShutdownPage() {
  const [hydrated, setHydrated] = useState(false);
  const [staffName, setStaffName] = useState<string | null>(null);
  const [staffId, setStaffId] = useState<string | null>(null);

  const [state, setState] = useState<ShutdownState>("idle");
  const [shutdownResponse, setShutdownResponse] = useState<VenueShutdownResponse | null>(null);
  const [errorMessage, setErrorMessage] = useState<string>("");
  const [devices, setDevices] = useState<DeviceStatus[]>(INITIAL_DEVICES);

  // Hydration: read sessionStorage only after mount (SSR can't access sessionStorage)
  useEffect(() => {
    setStaffName(sessionStorage.getItem("kiosk_staff_name"));
    setStaffId(sessionStorage.getItem("kiosk_staff_id"));
    setHydrated(true);
  }, []);

  // Visual shutdown progress timers after shutdown initiates
  useEffect(() => {
    if (state !== "shutting_down") return;

    // After 45s: pods done, POS in_progress
    const t1 = setTimeout(() => {
      setDevices([
        { label: "Pods 1–8", status: "done" },
        { label: "POS PC", status: "in_progress" },
        { label: "Server (.23)", status: "waiting" },
      ]);
    }, 45_000);

    // After 60s: POS done, server in_progress
    const t2 = setTimeout(() => {
      setDevices([
        { label: "Pods 1–8", status: "done" },
        { label: "POS PC", status: "done" },
        { label: "Server (.23)", status: "in_progress" },
      ]);
    }, 60_000);

    // After 120s: complete
    const t3 = setTimeout(() => {
      setDevices([
        { label: "Pods 1–8", status: "done" },
        { label: "POS PC", status: "done" },
        { label: "Server (.23)", status: "done" },
      ]);
      setState("complete");
    }, 120_000);

    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      clearTimeout(t3);
    };
  }, [state]);

  const handleConfirmShutdown = async () => {
    setState("auditing");
    setDevices(INITIAL_DEVICES);

    try {
      // 150s timeout — audit can take up to ~120s
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 150_000);

      let response: VenueShutdownResponse;
      try {
        response = await api.venueShutdown();
      } finally {
        clearTimeout(timeoutId);
      }

      setShutdownResponse(response);

      if (response.status === "shutting_down") {
        setDevices([
          { label: "Pods 1–8", status: "in_progress" },
          { label: "POS PC", status: "waiting" },
          { label: "Server (.23)", status: "waiting" },
        ]);
        setState("shutting_down");
      } else if (response.status === "blocked") {
        setState("audit_blocked");
      } else {
        setErrorMessage(response.message || "Unexpected error from server.");
        setState("error");
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Network error";
      setErrorMessage(msg);
      setState("error");
    }
  };

  // Auth gate: show nothing until hydrated, then show login if not authed
  if (!hydrated) {
    return <div className="h-screen bg-rp-black" />;
  }

  if (!staffName) {
    return (
      <StaffLoginScreen
        onAuthenticated={(id, name, token) => {
          setStaffId(id);
          setStaffName(name);
          sessionStorage.setItem("kiosk_staff_id", id);
          sessionStorage.setItem("kiosk_staff_name", name);
          if (token) sessionStorage.setItem("kiosk_staff_token", token);
        }}
      />
    );
  }

  return (
    <div className="min-h-screen bg-rp-black flex flex-col items-center justify-center p-6">
      {/* Branding */}
      <div className="text-center mb-8">
        <h1 className="text-3xl font-bold tracking-wide uppercase text-white">
          RACING<span className="text-rp-red">POINT</span>
        </h1>
        <p className="text-rp-grey text-sm mt-1 tracking-widest uppercase">
          Venue Shutdown
        </p>
      </div>

      {/* Main card */}
      <div className="w-full max-w-lg bg-rp-surface border border-rp-border rounded-2xl p-8">

        {/* ─── IDLE ─────────────────────────────────────────────────── */}
        {state === "idle" && (
          <div className="flex flex-col items-center gap-6">
            <div className="w-16 h-16 rounded-full bg-red-900/30 border border-red-600/30 flex items-center justify-center">
              <svg className="w-8 h-8 text-rp-red" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
              </svg>
            </div>

            <div className="text-center">
              <h2 className="text-xl font-bold text-white mb-2">Shutdown Racing Point</h2>
              <p className="text-rp-grey text-sm leading-relaxed">
                This will safely shut down all pods, POS, and server.<br />
                James (.27) will stay online to implement fixes on next boot.
              </p>
            </div>

            <button
              onClick={() => setState("confirming")}
              className="w-full py-4 bg-rp-red hover:bg-red-700 text-white font-bold text-lg rounded-xl transition-colors"
            >
              Shutdown Racing Point
            </button>

            <a
              href="/staff"
              className="text-rp-grey text-sm hover:text-white transition-colors"
            >
              &larr; Back to Staff Terminal
            </a>
          </div>
        )}

        {/* ─── CONFIRMING ───────────────────────────────────────────── */}
        {state === "confirming" && (
          <div className="flex flex-col items-center gap-6">
            <div className="w-16 h-16 rounded-full bg-amber-900/30 border border-amber-600/30 flex items-center justify-center">
              <svg className="w-8 h-8 text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>

            <div className="text-center">
              <h2 className="text-xl font-bold text-white mb-2">Shut down all Racing Point systems?</h2>
              <p className="text-rp-grey text-sm leading-relaxed">
                James (.27) will stay online to implement fixes on next boot.<br />
                A pre-shutdown audit will run first to check for critical issues.
              </p>
            </div>

            <div className="flex gap-3 w-full">
              <button
                onClick={handleConfirmShutdown}
                className="flex-1 py-3 bg-rp-red hover:bg-red-700 text-white font-semibold rounded-xl transition-colors"
              >
                Yes, Shutdown
              </button>
              <button
                onClick={() => setState("idle")}
                className="flex-1 py-3 border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded-xl transition-colors"
              >
                Cancel
              </button>
            </div>
          </div>
        )}

        {/* ─── AUDITING ─────────────────────────────────────────────── */}
        {state === "auditing" && (
          <div className="flex flex-col items-center gap-6">
            <div className="w-16 h-16 border-4 border-rp-red border-t-transparent rounded-full animate-spin" />
            <div className="text-center">
              <h2 className="text-xl font-bold text-white mb-2">Running pre-shutdown audit...</h2>
              <p className="text-rp-grey text-sm leading-relaxed">
                This checks for critical issues before shutdown. Please wait.<br />
                <span className="text-zinc-500 text-xs">May take up to 2 minutes.</span>
              </p>
            </div>
          </div>
        )}

        {/* ─── SHUTTING DOWN ────────────────────────────────────────── */}
        {state === "shutting_down" && (
          <div className="flex flex-col items-center gap-6">
            <div className="w-12 h-12 rounded-full bg-green-900/30 border border-green-600/30 flex items-center justify-center">
              <svg className="w-6 h-6 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
            </div>

            <div className="text-center">
              <h2 className="text-xl font-bold text-white mb-1">Audit passed</h2>
              <p className="text-rp-grey text-sm">Shutdown sequence initiated.</p>
            </div>

            <div className="w-full space-y-3">
              {devices.map((device) => (
                <div
                  key={device.label}
                  className="flex items-center justify-between bg-rp-black border border-rp-border rounded-xl px-4 py-3"
                >
                  <span className="text-white text-sm font-medium">{device.label}</span>
                  <DeviceStatusBadge status={device.status} />
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ─── COMPLETE ─────────────────────────────────────────────── */}
        {state === "complete" && (
          <div className="flex flex-col items-center gap-6">
            <div className="w-16 h-16 rounded-full bg-green-900/30 border border-green-600/30 flex items-center justify-center">
              <svg className="w-8 h-8 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <div className="text-center">
              <h2 className="text-xl font-bold text-white mb-2">Shutdown Complete</h2>
              <p className="text-rp-grey text-sm">All systems have been safely shut down.</p>
            </div>

            <div className="w-full space-y-3">
              {devices.map((device) => (
                <div
                  key={device.label}
                  className="flex items-center justify-between bg-rp-black border border-rp-border rounded-xl px-4 py-3"
                >
                  <span className="text-white text-sm font-medium">{device.label}</span>
                  <DeviceStatusBadge status={device.status} />
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ─── AUDIT BLOCKED ────────────────────────────────────────── */}
        {state === "audit_blocked" && shutdownResponse && (
          <div className="flex flex-col items-center gap-6">
            <div className="w-16 h-16 rounded-full bg-red-900/30 border border-red-600/30 flex items-center justify-center">
              <svg className="w-8 h-8 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </div>

            <div className="text-center w-full">
              <h2 className="text-xl font-bold text-white mb-3">Cannot Shutdown</h2>

              {shutdownResponse.reason === "billing_active" && (
                <p className="text-amber-400 text-sm leading-relaxed">
                  {shutdownResponse.active_sessions ?? "Some"} billing session
                  {(shutdownResponse.active_sessions ?? 0) !== 1 ? "s" : ""} still active.<br />
                  End all sessions first before shutting down.
                </p>
              )}

              {shutdownResponse.reason === "audit_failed" && (
                <div className="text-left mt-3">
                  <p className="text-red-400 text-sm mb-2">Pre-shutdown audit found critical issues:</p>
                  {shutdownResponse.output && (
                    <pre className="bg-rp-black border border-rp-border rounded-lg p-3 text-xs text-zinc-400 overflow-x-auto max-h-40 overflow-y-auto whitespace-pre-wrap">
                      {shutdownResponse.output.slice(0, 800)}
                    </pre>
                  )}
                </div>
              )}

              {shutdownResponse.reason === "james_offline" && (
                <p className="text-amber-400 text-sm leading-relaxed">
                  James is offline. Contact Bono for remote shutdown.
                </p>
              )}

              {!shutdownResponse.reason && (
                <p className="text-rp-grey text-sm">{shutdownResponse.message}</p>
              )}
            </div>

            <div className="flex gap-3 w-full">
              <button
                onClick={() => setState("idle")}
                className="flex-1 py-3 border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded-xl transition-colors"
              >
                Return to Shutdown
              </button>
              <a
                href="/staff"
                className="flex-1 py-3 text-center border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded-xl transition-colors text-sm font-medium"
              >
                Return to Staff Terminal
              </a>
            </div>
          </div>
        )}

        {/* ─── ERROR ────────────────────────────────────────────────── */}
        {state === "error" && (
          <div className="flex flex-col items-center gap-6">
            <div className="w-16 h-16 rounded-full bg-red-900/30 border border-red-600/30 flex items-center justify-center">
              <svg className="w-8 h-8 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>

            <div className="text-center">
              <h2 className="text-xl font-bold text-white mb-2">Shutdown Failed</h2>
              <p className="text-rp-grey text-sm">{errorMessage || "An unexpected error occurred."}</p>
            </div>

            <div className="flex gap-3 w-full">
              <button
                onClick={() => { setState("idle"); setErrorMessage(""); }}
                className="flex-1 py-3 bg-rp-red hover:bg-red-700 text-white font-semibold rounded-xl transition-colors"
              >
                Try Again
              </button>
              <a
                href="/staff"
                className="flex-1 py-3 text-center border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded-xl transition-colors text-sm font-medium"
              >
                Staff Terminal
              </a>
            </div>
          </div>
        )}
      </div>

      {/* Footer: staff info */}
      <p className="mt-6 text-xs text-zinc-600">
        Logged in as <span className="text-zinc-500">{staffName}</span>
        {staffId && <span className="text-zinc-600"> ({staffId})</span>}
      </p>
    </div>
  );
}

// ─── Device Status Badge ─────────────────────────────────────────────────────

function DeviceStatusBadge({ status }: { status: DeviceStatus["status"] }) {
  if (status === "waiting") {
    return <span className="text-xs text-zinc-500 font-medium">Waiting...</span>;
  }
  if (status === "in_progress") {
    return (
      <span className="flex items-center gap-1.5 text-xs text-amber-400 font-medium">
        <span className="w-3 h-3 border-2 border-amber-400 border-t-transparent rounded-full animate-spin" />
        Shutting down...
      </span>
    );
  }
  return (
    <span className="flex items-center gap-1.5 text-xs text-green-400 font-medium">
      <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
      </svg>
      Done
    </span>
  );
}
