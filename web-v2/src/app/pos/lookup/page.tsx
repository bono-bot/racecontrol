"use client";

/**
 * /v2/pos/lookup — POS .130 customer lookup page.
 *
 * PACT-20260506-001 Phase 1 wire-up Session 4 — UI scaffolding.
 *
 * Wires the 5 lookup components (PhoneLookupInput / ProfilePreviewCard /
 * WalkInGuestDropdown / NotFoundCTA / LookupErrorBanner) into a state
 * machine: idle → looking_up → (found | not_found | error) | walkin_selected.
 *
 * Path note: file lives at app/pos/lookup/page.tsx (NOT app/v2/pos/lookup) —
 * Next.js next.config.ts has `basePath: "/v2"` which auto-prepends /v2 to all
 * routes. PLAN.md Session 4 cited app/v2/pos/lookup/page.tsx but that would
 * produce URL /v2/v2/pos/lookup (double-prefix). Corrected here to ship the
 * canonical /v2/pos/lookup URL.
 *
 * API call: fetch("/api/v1/cirs/lookup") — Next.js route handler proxy from
 * web-v2 → racecontrol :8080 not yet wired (Session 5 scope). Page state
 * machine works in isolation; actual lookup returns whatever the proxy
 * (when wired) returns, or 404 until then. NOT TESTED with live API.
 */

import { useState } from "react";
import PhoneLookupInput from "@/components/v2/pos/PhoneLookupInput";
import ProfilePreviewCard from "@/components/v2/pos/ProfilePreviewCard";
import WalkInGuestDropdown from "@/components/v2/pos/WalkInGuestDropdown";
import NotFoundCTA from "@/components/v2/pos/NotFoundCTA";
import LookupErrorBanner from "@/components/v2/pos/LookupErrorBanner";
import type {
  CirsLookupRequest,
  CirsLookupResponse,
  ProfilePreview,
} from "@/lib/types/cirs-lookup";
import styles from "./page.module.css";

type ViewState =
  | { kind: "idle" }
  | { kind: "walkin" }
  | { kind: "looking_up"; searchedPhone?: string }
  | { kind: "found"; profile: ProfilePreview }
  | { kind: "not_found"; searchedPhone: string }
  | { kind: "error"; code: string; message: string };

const LOOKUP_ENDPOINT = "/api/v1/cirs/lookup";

async function callLookup(
  body: CirsLookupRequest,
): Promise<CirsLookupResponse> {
  try {
    const res = await fetch(LOOKUP_ENDPOINT, {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      return {
        status: "error",
        code: `HTTP_${res.status}`,
        message: `Lookup endpoint returned ${res.status}. Proxy wiring may be pending (Session 5).`,
      };
    }
    return (await res.json()) as CirsLookupResponse;
  } catch (e) {
    return {
      status: "error",
      code: "NETWORK_ERROR",
      message:
        e instanceof Error
          ? e.message
          : "Lookup endpoint unreachable from POS browser.",
    };
  }
}

export default function PosLookupPage() {
  const [view, setView] = useState<ViewState>({ kind: "idle" });

  const lookup = async (body: CirsLookupRequest, searchedPhone?: string) => {
    setView({ kind: "looking_up", searchedPhone });
    const res = await callLookup(body);
    if (res.status === "found") {
      setView({ kind: "found", profile: res.profile });
    } else if (res.status === "not_found") {
      setView({
        kind: "not_found",
        searchedPhone: searchedPhone ?? "(unknown)",
      });
    } else {
      setView({ kind: "error", code: res.code, message: res.message });
    }
  };

  const handlePhoneSubmit = (phone: string) => {
    void lookup({ method: "phone", phone }, phone);
  };

  const handleWalkInSelect = (id: 1 | 2) => {
    // Inner field is `guest_id` to match Rust canonical wire format
    // (Session 7 MMA EXECUTE path A — earlier draft sent `walk_in_guest_id`
    // which 400'd at the racecontrol handler).
    void lookup({ method: "walk_in_guest_id", guest_id: id });
  };

  const reset = () => setView({ kind: "idle" });

  return (
    <main className={styles.shell}>
      <header className={styles.header}>
        <h1 className={styles.title}>POS — Customer lookup</h1>
        <p className={styles.subtitle}>
          Type the customer&apos;s phone number, or use a Walk-In Guest slot if
          they&apos;re not registered.
        </p>
      </header>

      {(view.kind === "idle" || view.kind === "looking_up") && (
        <PhoneLookupInput
          onSubmit={handlePhoneSubmit}
          disabled={view.kind === "looking_up"}
        />
      )}

      {view.kind === "looking_up" && (
        <p className={styles.statusLine}>Looking up customer…</p>
      )}

      {view.kind === "found" && (
        <>
          <ProfilePreviewCard profile={view.profile} />
          <button
            type="button"
            onClick={reset}
            style={{
              padding: "0.625rem 1rem",
              background: "transparent",
              color: "var(--rp-text-muted)",
              border: "1px solid var(--rp-border)",
              borderRadius: 6,
              cursor: "pointer",
              alignSelf: "flex-start",
            }}
          >
            Lookup another customer
          </button>
        </>
      )}

      {view.kind === "not_found" && (
        <NotFoundCTA
          searchedPhone={view.searchedPhone}
          onUseWalkIn={() => setView({ kind: "walkin" })}
          onRegisterNew={() => {
            // Deeplink target finalizes Session 5 — placeholder external nav
            window.location.href = "/v2/pwa/register";
          }}
        />
      )}

      {view.kind === "error" && (
        <LookupErrorBanner
          code={view.code}
          message={view.message}
          onRetry={reset}
          onUseWalkIn={() => setView({ kind: "walkin" })}
        />
      )}

      {(view.kind === "walkin" || view.kind === "not_found") && (
        <WalkInGuestDropdown onSelect={handleWalkInSelect} />
      )}
    </main>
  );
}
