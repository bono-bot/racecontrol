"use client";

import { useEffect, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { FriendInfo, FriendRequestInfo } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

export default function FriendsPage() {
  const router = useRouter();
  const [friends, setFriends] = useState<FriendInfo[]>([]);
  const [incoming, setIncoming] = useState<FriendRequestInfo[]>([]);
  const [outgoing, setOutgoing] = useState<FriendRequestInfo[]>([]);
  const [addInput, setAddInput] = useState("");
  const [sending, setSending] = useState(false);
  const [presence, setPresence] = useState<string>("hidden");
  const [message, setMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [tab, setTab] = useState<"friends" | "requests">("friends");

  const loadData = useCallback(async () => {
    try {
      const [fRes, rRes, pRes] = await Promise.all([
        api.friends(),
        api.friendRequests(),
        api.profile(),
      ]);
      if (fRes.friends) setFriends(fRes.friends);
      if (rRes.incoming) setIncoming(rRes.incoming);
      if (rRes.outgoing) setOutgoing(rRes.outgoing);
      if (pRes.driver) {
        // Presence isn't in profile yet — we'll use a local state
      }
    } catch {
      // network error
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    loadData();
  }, [router, loadData]);

  async function handleSendRequest() {
    if (!addInput.trim()) return;
    setSending(true);
    setMessage(null);
    try {
      const res = await api.sendFriendRequest(addInput.trim());
      if (res.error) {
        setMessage(res.error);
      } else {
        setMessage("Friend request sent!");
        setAddInput("");
        loadData();
      }
    } catch {
      setMessage("Network error");
    } finally {
      setSending(false);
    }
  }

  async function handleAccept(requestId: string) {
    await api.acceptFriendRequest(requestId);
    loadData();
  }

  async function handleReject(requestId: string) {
    await api.rejectFriendRequest(requestId);
    loadData();
  }

  async function handleRemove(driverId: string) {
    await api.removeFriend(driverId);
    loadData();
  }

  async function togglePresence() {
    const next = presence === "online" ? "hidden" : "online";
    await api.setPresence(next);
    setPresence(next);
  }

  if (loading) {
    return (
      <div className="min-h-screen pb-20 flex items-center justify-center">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  const pendingCount = incoming.length;

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <div className="flex items-center justify-between mb-6">
          <h1 className="text-2xl font-bold text-white">Friends</h1>
          {/* Presence toggle */}
          <button
            onClick={togglePresence}
            className="flex items-center gap-2 bg-rp-card border border-rp-border rounded-lg px-3 py-1.5"
          >
            <div
              className={`w-2 h-2 rounded-full ${
                presence === "online" ? "bg-green-500" : "bg-neutral-600"
              }`}
            />
            <span className="text-xs text-neutral-300">
              {presence === "online" ? "Online" : "Hidden"}
            </span>
          </button>
        </div>

        {/* Add friend */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-6">
          <p className="text-sm text-rp-grey mb-3">Add Friend</p>
          <div className="flex gap-2">
            <input
              type="text"
              value={addInput}
              onChange={(e) => setAddInput(e.target.value)}
              placeholder="Phone or ID (e.g. RP002)"
              className="flex-1 bg-neutral-800 border border-neutral-700 rounded-lg px-3 py-2 text-sm text-white placeholder:text-neutral-500 outline-none focus:border-rp-red"
            />
            <button
              onClick={handleSendRequest}
              disabled={sending || !addInput.trim()}
              className="bg-rp-red text-white font-semibold px-4 py-2 rounded-lg text-sm disabled:opacity-50"
            >
              {sending ? "..." : "Add"}
            </button>
          </div>
          {message && (
            <p
              className={`text-xs mt-2 ${
                message.includes("sent") || message.includes("accepted")
                  ? "text-green-400"
                  : "text-red-400"
              }`}
            >
              {message}
            </p>
          )}
        </div>

        {/* Tabs */}
        <div className="flex gap-2 mb-4">
          <button
            onClick={() => setTab("friends")}
            className={`px-4 py-2 rounded-lg text-sm font-medium ${
              tab === "friends"
                ? "bg-rp-red text-white"
                : "bg-rp-card text-rp-grey border border-rp-border"
            }`}
          >
            Friends ({friends.length})
          </button>
          <button
            onClick={() => setTab("requests")}
            className={`px-4 py-2 rounded-lg text-sm font-medium relative ${
              tab === "requests"
                ? "bg-rp-red text-white"
                : "bg-rp-card text-rp-grey border border-rp-border"
            }`}
          >
            Requests
            {pendingCount > 0 && (
              <span className="absolute -top-1 -right-1 bg-rp-red text-white text-[10px] font-bold w-5 h-5 rounded-full flex items-center justify-center">
                {pendingCount}
              </span>
            )}
          </button>
        </div>

        {/* Friends list */}
        {tab === "friends" && (
          <div className="space-y-2">
            {friends.length === 0 ? (
              <p className="text-rp-grey text-sm text-center py-8">
                No friends yet. Add someone above!
              </p>
            ) : (
              friends.map((f) => (
                <div
                  key={f.driver_id}
                  className="bg-rp-card border border-rp-border rounded-xl p-4 flex items-center justify-between"
                >
                  <div className="flex items-center gap-3">
                    <div className="relative">
                      <div className="w-10 h-10 rounded-full bg-neutral-700 flex items-center justify-center">
                        <span className="text-sm font-bold text-neutral-300">
                          {f.name.charAt(0).toUpperCase()}
                        </span>
                      </div>
                      <div
                        className={`absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2 border-rp-card ${
                          f.is_online ? "bg-green-500" : "bg-neutral-600"
                        }`}
                      />
                    </div>
                    <div>
                      <p className="text-sm font-medium text-white">{f.name}</p>
                      {f.customer_id && (
                        <p className="text-xs text-rp-grey font-mono">
                          {f.customer_id}
                        </p>
                      )}
                    </div>
                  </div>
                  <button
                    onClick={() => handleRemove(f.driver_id)}
                    className="text-xs text-neutral-500 hover:text-red-400 transition-colors"
                  >
                    Remove
                  </button>
                </div>
              ))
            )}
          </div>
        )}

        {/* Requests */}
        {tab === "requests" && (
          <div className="space-y-4">
            {/* Incoming */}
            {incoming.length > 0 && (
              <div>
                <p className="text-xs text-rp-grey mb-2 uppercase tracking-wider">
                  Incoming
                </p>
                <div className="space-y-2">
                  {incoming.map((r) => (
                    <div
                      key={r.id}
                      className="bg-rp-card border border-rp-border rounded-xl p-4 flex items-center justify-between"
                    >
                      <div>
                        <p className="text-sm font-medium text-white">
                          {r.driver_name}
                        </p>
                        {r.customer_id && (
                          <p className="text-xs text-rp-grey font-mono">
                            {r.customer_id}
                          </p>
                        )}
                      </div>
                      <div className="flex gap-2">
                        <button
                          onClick={() => handleAccept(r.id)}
                          className="bg-rp-red text-white px-3 py-1.5 rounded-lg text-xs font-semibold"
                        >
                          Accept
                        </button>
                        <button
                          onClick={() => handleReject(r.id)}
                          className="bg-neutral-800 text-neutral-400 px-3 py-1.5 rounded-lg text-xs"
                        >
                          Decline
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Outgoing */}
            {outgoing.length > 0 && (
              <div>
                <p className="text-xs text-rp-grey mb-2 uppercase tracking-wider">
                  Sent
                </p>
                <div className="space-y-2">
                  {outgoing.map((r) => (
                    <div
                      key={r.id}
                      className="bg-rp-card border border-rp-border rounded-xl p-3 flex items-center justify-between"
                    >
                      <div>
                        <p className="text-sm text-neutral-300">
                          {r.driver_name}
                        </p>
                        {r.customer_id && (
                          <p className="text-xs text-rp-grey font-mono">
                            {r.customer_id}
                          </p>
                        )}
                      </div>
                      <span className="text-xs text-amber-400">Pending</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {incoming.length === 0 && outgoing.length === 0 && (
              <p className="text-rp-grey text-sm text-center py-8">
                No pending requests
              </p>
            )}
          </div>
        )}
      </div>
      <BottomNav />
    </div>
  );
}
