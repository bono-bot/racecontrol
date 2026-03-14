"use client";

import { useEffect, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { GroupSessionInfo } from "@/lib/api";

export default function GroupSessionPage() {
  const router = useRouter();
  const [group, setGroup] = useState<GroupSessionInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [acting, setActing] = useState(false);
  const [driverId, setDriverId] = useState<string | null>(null);

  const loadGroup = useCallback(async () => {
    try {
      const [gRes, pRes] = await Promise.all([
        api.groupSession(),
        api.profile(),
      ]);
      if (gRes.group_session) {
        setGroup(gRes.group_session);
      }
      if (pRes.driver) {
        setDriverId(pRes.driver.id);
      }
    } catch {
      // network error
    } finally {
      setLoading(false);
    }
  }, [router]);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    loadGroup();
    const interval = setInterval(loadGroup, 3000);
    return () => clearInterval(interval);
  }, [router, loadGroup]);

  async function handleAccept() {
    if (!group) return;
    setActing(true);
    try {
      const res = await api.acceptGroupInvite(group.id);
      if (res.error) {
        alert(res.error);
      }
      loadGroup();
    } finally {
      setActing(false);
    }
  }

  async function handleDecline() {
    if (!group) return;
    setActing(true);
    try {
      await api.declineGroupInvite(group.id);
      router.push("/dashboard");
    } finally {
      setActing(false);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (!group) {
    return (
      <div className="px-4 pt-12 pb-24 max-w-lg mx-auto text-center">
        <p className="text-rp-grey text-sm mb-1">Multiplayer</p>
        <h1 className="text-2xl font-bold text-white mb-4">
          No Active Group Session
        </h1>
        <p className="text-neutral-400 text-sm mb-8">
          Create a multiplayer session to race against your friends on LAN.
        </p>
        <button
          onClick={() => router.push("/book/multiplayer")}
          className="bg-rp-red hover:bg-rp-red/90 text-white font-semibold px-8 py-3 rounded-xl transition-colors"
        >
          Create Multiplayer Session
        </button>
        <button
          onClick={() => router.push("/book")}
          className="block mx-auto mt-4 text-rp-grey text-sm"
        >
          Back to Booking
        </button>
      </div>
    );
  }

  const myMember = group.members.find((m) => m.driver_id === driverId);
  const isPending = myMember?.status === "pending";
  const isHost = myMember?.role === "host";
  const allValidated = group.status === "all_validated";

  return (
    <div className="px-4 pt-12 pb-24 max-w-lg mx-auto text-center">
      {/* Header */}
      <p className="text-rp-grey text-sm mb-1">Multiplayer Session</p>
      <h1 className="text-2xl font-bold text-white mb-2">
        {group.experience_name}
      </h1>
      <p className="text-rp-grey text-xs mb-6">
        Hosted by {group.host_name} &middot; {group.pricing_tier_name}
      </p>

      {/* Session Info Cards */}
      {group.track && (
        <div className="grid grid-cols-3 gap-2 mb-6">
          <div className="bg-rp-card border border-rp-border rounded-xl p-3 text-center">
            <p className="text-xs text-rp-grey mb-1">Track</p>
            <p className="text-sm font-semibold text-white truncate">
              {formatDisplayName(group.track)}
            </p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded-xl p-3 text-center">
            <p className="text-xs text-rp-grey mb-1">Car</p>
            <p className="text-sm font-semibold text-white truncate">
              {formatDisplayName(group.car || '')}
            </p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded-xl p-3 text-center">
            <p className="text-xs text-rp-grey mb-1">AI Opponents</p>
            <p className="text-sm font-semibold text-white">
              {group.ai_count ?? 0}
            </p>
          </div>
        </div>
      )}

      {/* Shared PIN */}
      {(myMember?.status === "accepted" || myMember?.status === "validated" || isHost) && (
        <div className="bg-rp-card border border-rp-border rounded-2xl p-6 mb-8 inline-block">
          <p className="text-rp-grey text-xs mb-2">Shared PIN</p>
          <p className="text-5xl font-mono font-bold text-rp-red tracking-wider">
            {group.shared_pin}
          </p>
          <p className="text-neutral-500 text-xs mt-2">
            Enter this PIN on your assigned pod
          </p>
        </div>
      )}

      {/* Pending invite */}
      {isPending && !isHost && (
        <div className="bg-rp-red/10 border border-rp-red/30 rounded-xl p-6 mb-8">
          <p className="text-white font-semibold mb-2">
            {group.host_name} invited you to race!
          </p>
          <p className="text-neutral-400 text-sm mb-4">
            {group.experience_name} &middot; {group.pricing_tier_name}
          </p>
          <div className="flex gap-3 justify-center">
            <button
              onClick={handleAccept}
              disabled={acting}
              className="bg-rp-red text-white font-semibold px-6 py-3 rounded-xl disabled:opacity-50"
            >
              {acting ? "..." : "Accept & Pay"}
            </button>
            <button
              onClick={handleDecline}
              disabled={acting}
              className="bg-neutral-800 text-neutral-400 px-6 py-3 rounded-xl disabled:opacity-50"
            >
              Decline
            </button>
          </div>
        </div>
      )}

      {/* Members list */}
      <div className="mb-8">
        <p className="text-xs text-rp-grey mb-3 uppercase tracking-wider">
          Players
        </p>
        <div className="space-y-2">
          {group.members.map((m) => (
            <div
              key={m.driver_id}
              className="bg-rp-card border border-rp-border rounded-xl p-4 flex items-center justify-between"
            >
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-full bg-neutral-700 flex items-center justify-center">
                  <span className="text-sm font-bold text-neutral-300">
                    {m.driver_name.charAt(0).toUpperCase()}
                  </span>
                </div>
                <div className="text-left">
                  <p className="text-sm font-medium text-white">
                    {m.driver_name}
                    {m.role === "host" && (
                      <span className="ml-1 text-xs text-rp-red">(Host)</span>
                    )}
                  </p>
                  {m.pod_number && (
                    <p className="text-xs text-rp-grey">Pod {m.pod_number}</p>
                  )}
                </div>
              </div>
              <MemberStatus status={m.status} />
            </div>
          ))}
        </div>
      </div>

      {/* Status message */}
      {allValidated ? (
        <div className="bg-emerald-900/30 border border-emerald-500/30 rounded-xl p-4">
          <p className="text-emerald-400 font-semibold">
            All players checked in!
          </p>
          <p className="text-neutral-400 text-sm mt-1">
            Race is starting on all pods...
          </p>
        </div>
      ) : group.status === "active" ? (
        <div className="flex items-center justify-center gap-2">
          <div className="w-3 h-3 bg-amber-500 rounded-full animate-pulse" />
          <span className="text-amber-400 text-sm">
            {(() => {
              const remaining = group.members.filter(
                (m) => m.status !== "validated"
              ).length;
              return remaining > 0
                ? `Waiting for ${remaining} player${remaining !== 1 ? "s" : ""} to check in...`
                : "Waiting for all players to check in...";
            })()}
          </span>
        </div>
      ) : null}
    </div>
  );
}

function formatDisplayName(id: string): string {
  return id
    .replace(/^ks_/, "")
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

function MemberStatus({ status }: { status: string }) {
  switch (status) {
    case "validated":
      return (
        <span className="flex items-center gap-1.5 text-xs text-green-400">
          <div className="w-2 h-2 bg-green-500 rounded-full" />
          Checked in
        </span>
      );
    case "accepted":
      return (
        <span className="flex items-center gap-1.5 text-xs text-amber-400">
          <div className="w-2 h-2 bg-amber-500 rounded-full" />
          Accepted
        </span>
      );
    case "pending":
      return (
        <span className="flex items-center gap-1.5 text-xs text-neutral-500">
          <div className="w-2 h-2 bg-neutral-600 rounded-full" />
          Invited
        </span>
      );
    case "declined":
      return (
        <span className="text-xs text-neutral-600">Declined</span>
      );
    default:
      return <span className="text-xs text-neutral-500">{status}</span>;
  }
}
