"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import type {
  Pod,
  TelemetryFrame,
  Lap,
  BillingSession,
  BillingWarning,
  GameLaunchInfo,
  AuthTokenInfo,
  PodActivityEntry,
  PendingSplitContinuation,
  DeployProgressEvent,
  DeployState,
  AcServerInfo,
  MultiplayerGroupStatus,
} from "@/lib/types";

const WS_BASE =
  process.env.NEXT_PUBLIC_WS_URL ||
  (typeof window !== "undefined"
    ? `ws://${window.location.hostname}:8080/ws/dashboard`
    : "ws://localhost:8080/ws/dashboard");
// ACCEPTED RISK: NEXT_PUBLIC_WS_TOKEN is exposed in the client bundle.
// This is a LAN-only kiosk system with no public internet exposure.
// The token is sent via Sec-WebSocket-Protocol header instead of URL query
// string to avoid logging in server access logs and browser history.
const WS_TOKEN = process.env.NEXT_PUBLIC_WS_TOKEN || "";
const WS_URL = WS_BASE;

interface DashboardEvent {
  event: string;
  data: unknown;
}

interface AssistanceRequest {
  pod_id: string;
  driver_name: string;
  game: string;
  reason: string;
  timestamp: number;
}

export interface GameLaunchRequest {
  pod_id: string;
  sim_type: string;
  driver_name: string;
  request_id: string;
}

export type { AssistanceRequest };

export function useKioskSocket() {
  const ws = useRef<WebSocket | null>(null);
  const disconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reconnectAttemptRef = useRef(0);
  const [connected, setConnected] = useState(false);
  const [pods, setPods] = useState<Map<string, Pod>>(new Map());
  const [latestTelemetry, setLatestTelemetry] = useState<Map<string, TelemetryFrame>>(new Map());
  const [recentLaps, setRecentLaps] = useState<Lap[]>([]);
  const [billingTimers, setBillingTimers] = useState<Map<string, BillingSession>>(new Map());
  const [billingWarnings, setBillingWarnings] = useState<BillingWarning[]>([]);
  const [gameStates, setGameStates] = useState<Map<string, GameLaunchInfo>>(new Map());
  const [pendingAuthTokens, setPendingAuthTokens] = useState<Map<string, AuthTokenInfo>>(new Map());
  const [assistanceRequests, setAssistanceRequests] = useState<AssistanceRequest[]>([]);
  const [gameLaunchRequests, setGameLaunchRequests] = useState<GameLaunchRequest[]>([]);
  const [cameraFocus, setCameraFocus] = useState<{ pod_id: string; driver_name: string; reason: string } | null>(null);
  const [activityLog, setActivityLog] = useState<PodActivityEntry[]>([]);
  const [pendingSplitContinuation, setPendingSplitContinuation] = useState<PendingSplitContinuation | null>(null);
  const [deployStates, setDeployStates] = useState<Map<string, DeployState>>(new Map());
  const [acServerInfo, setAcServerInfo] = useState<AcServerInfo | null>(null);
  const [multiplayerGroup, setMultiplayerGroup] = useState<MultiplayerGroupStatus | null>(null);

  const sendCommand = useCallback(
    (command: string, data: Record<string, unknown>) => {
      if (ws.current?.readyState === WebSocket.OPEN) {
        ws.current.send(JSON.stringify({ command, data }));
      }
    },
    []
  );

  const connect = useCallback(() => {
    if (ws.current?.readyState === WebSocket.OPEN) return;

    const socket = WS_TOKEN
      ? new WebSocket(WS_URL, [WS_TOKEN])
      : new WebSocket(WS_URL);

    socket.onopen = () => {
      // Clear any pending disconnect timer -- we reconnected in time
      if (disconnectTimerRef.current !== null) {
        clearTimeout(disconnectTimerRef.current);
        disconnectTimerRef.current = null;
      }
      reconnectAttemptRef.current = 0;
      setConnected(true);
      console.log("[Kiosk] Connected to RaceControl");
    };

    socket.onmessage = (event) => {
      try {
        const msg: DashboardEvent = JSON.parse(event.data);

        switch (msg.event) {
          case "pod_list": {
            const podList = msg.data as Pod[];
            const map = new Map<string, Pod>();
            podList.forEach((p) => map.set(p.id, p));
            setPods(map);
            break;
          }
          case "pod_update": {
            const pod = msg.data as Pod;
            setPods((prev) => {
              const next = new Map(prev);
              next.set(pod.id, pod);
              return next;
            });
            break;
          }
          case "telemetry": {
            const frame = msg.data as TelemetryFrame;
            setLatestTelemetry((prev) => {
              const next = new Map(prev);
              next.set(frame.pod_id, frame);
              return next;
            });
            break;
          }
          case "lap_completed": {
            const lap = msg.data as Lap;
            setRecentLaps((prev) => [lap, ...prev].slice(0, 50));
            break;
          }
          case "billing_session_list": {
            const sessions = msg.data as BillingSession[];
            const map = new Map<string, BillingSession>();
            sessions.forEach((s) => map.set(s.pod_id, s));
            setBillingTimers(map);
            break;
          }
          case "billing_tick": {
            const session = msg.data as BillingSession;
            setBillingTimers((prev) => {
              const next = new Map(prev);
              next.set(session.pod_id, session);
              return next;
            });
            break;
          }
          case "billing_session_changed": {
            const session = msg.data as BillingSession;
            const TERMINAL_STATUSES = ["completed", "cancelled", "ended_early", "cancelled_no_playable"] as const;
            setBillingTimers((prev) => {
              const next = new Map(prev);
              if ((TERMINAL_STATUSES as readonly string[]).includes(session.status)) {
                next.delete(session.pod_id);

                // Detect split session completion — trigger between-sessions flow
                if (
                  session.status === "completed" &&
                  session.split_count &&
                  session.split_count > 1 &&
                  session.current_split_number != null &&
                  session.current_split_number < session.split_count
                ) {
                  setPendingSplitContinuation({
                    pod_id: session.pod_id,
                    driver_id: session.driver_id,
                    driver_name: session.driver_name,
                    split_count: session.split_count,
                    current_split_number: session.current_split_number,
                    split_duration_minutes: session.split_duration_minutes ?? 0,
                  });
                }
              } else {
                next.set(session.pod_id, session);
              }
              return next;
            });
            break;
          }
          case "billing_warning": {
            const w = msg.data as {
              billing_session_id: string;
              pod_id: string;
              remaining_seconds: number;
            };
            const warning: BillingWarning = {
              sessionId: w.billing_session_id,
              podId: w.pod_id,
              remaining: w.remaining_seconds,
              timestamp: Date.now(),
            };
            setBillingWarnings((prev) => [...prev, warning]);
            setTimeout(() => {
              setBillingWarnings((prev) =>
                prev.filter((bw) => bw.timestamp !== warning.timestamp)
              );
            }, 10000);
            break;
          }
          case "game_session_list": {
            const games = msg.data as GameLaunchInfo[];
            const map = new Map<string, GameLaunchInfo>();
            games.forEach((g) => map.set(g.pod_id, g));
            setGameStates(map);
            break;
          }
          case "game_state_changed": {
            const info = msg.data as GameLaunchInfo;
            setGameStates((prev) => {
              const next = new Map(prev);
              if (info.game_state === "idle") {
                next.delete(info.pod_id);
              } else {
                next.set(info.pod_id, info);
              }
              return next;
            });
            break;
          }
          case "auth_token_created": {
            const token = msg.data as AuthTokenInfo;
            setPendingAuthTokens((prev) => {
              const next = new Map(prev);
              next.set(token.pod_id, token);
              return next;
            });
            break;
          }
          case "auth_token_consumed": {
            const d = msg.data as { token_id: string; pod_id: string };
            setPendingAuthTokens((prev) => {
              const next = new Map(prev);
              next.delete(d.pod_id);
              return next;
            });
            break;
          }
          case "auth_token_cleared": {
            const d = msg.data as { token_id: string; pod_id: string };
            setPendingAuthTokens((prev) => {
              const next = new Map(prev);
              next.delete(d.pod_id);
              return next;
            });
            break;
          }
          case "assistance_needed": {
            const d = msg.data as {
              pod_id: string;
              driver_name: string;
              game: string;
              reason: string;
            };
            setAssistanceRequests((prev) => [
              ...prev,
              { ...d, timestamp: Date.now() },
            ]);
            break;
          }
          case "pod_reservation_changed": {
            const d = msg.data as {
              reservation_id: string;
              driver_id: string;
              pod_id: string;
              status: string;
            };
            console.log("[Kiosk] Pod reservation changed:", d);
            break;
          }
          case "camera_focus_update": {
            const d = msg.data as {
              pod_id: string;
              driver_name: string;
              reason: string;
            };
            setCameraFocus(d.pod_id ? d : null);
            break;
          }
          case "pod_activity": {
            const entry = msg.data as PodActivityEntry;
            setActivityLog((prev) => [entry, ...prev].slice(0, 500));
            break;
          }
          case "pod_activity_list": {
            const entries = msg.data as PodActivityEntry[];
            setActivityLog(entries);
            break;
          }
          case "deploy_progress": {
            const event = msg.data as DeployProgressEvent;
            setDeployStates((prev) => {
              const next = new Map(prev);
              next.set(event.pod_id, event.state);
              return next;
            });
            break;
          }
          case "ac_server_update": {
            const info = msg.data as AcServerInfo;
            setAcServerInfo(info.status === "stopped" ? null : info);
            break;
          }
          case "group_session_all_validated": {
            const data = msg.data as MultiplayerGroupStatus;
            setMultiplayerGroup(data);
            break;
          }
          case "GameLaunchRequested": {
            const req = msg.data as GameLaunchRequest;
            setGameLaunchRequests((prev) => [req, ...prev]);
            // Auto-expire after 60 seconds
            setTimeout(() => {
              setGameLaunchRequests((prev) =>
                prev.filter((r) => r.request_id !== req.request_id)
              );
            }, 60 * 1000);
            break;
          }
        }
      } catch (e) {
        console.warn("[Kiosk] Parse error:", e);
      }
    };

    socket.onclose = () => {
      // WS-HARDEN: Exponential backoff with jitter: 1s base, 30s max, cap at 20 retries
      const attempt = reconnectAttemptRef.current;
      if (attempt >= 20) {
        console.error("[Kiosk] Max reconnect attempts (20) reached — giving up. Refresh page to retry.");
        setConnected(false);
        return;
      }
      const baseDelay = Math.min(1000 * Math.pow(2, attempt), 30_000);
      const jitter = Math.random() * baseDelay * 0.3;
      const delay = Math.round(baseDelay + jitter);
      reconnectAttemptRef.current = attempt + 1;
      console.log(`[Kiosk] Disconnected, retrying in ${delay}ms (attempt ${attempt + 1}/20)...`);
      // Two-phase UI debounce:
      // 1. After 5s: show "Reconnecting..." (soft indicator, no alarm)
      // 2. After 15s: show "Disconnected" (hard indicator, something is wrong)
      // This prevents false flashes during game launch CPU spikes while still
      // giving staff timely feedback if the connection is actually lost.
      if (disconnectTimerRef.current === null) {
        disconnectTimerRef.current = setTimeout(() => {
          setConnected(false);
          disconnectTimerRef.current = null;
          console.log("[Kiosk] 15s debounce expired -- marking disconnected");
        }, 15_000);
      }
      // Retry connection with exponential backoff (separate from UI debounce)
      setTimeout(connect, delay);
    };

    socket.onerror = () => {
      socket.close();
    };

    ws.current = socket;
  }, []);

  useEffect(() => {
    connect();
    return () => {
      ws.current?.close();
      // Clean up debounce timer on unmount
      if (disconnectTimerRef.current !== null) {
        clearTimeout(disconnectTimerRef.current);
        disconnectTimerRef.current = null;
      }
    };
  }, [connect]);

  const dismissAssistance = useCallback((podId: string) => {
    setAssistanceRequests((prev) => prev.filter((r) => r.pod_id !== podId));
  }, []);

  const dismissGameRequest = useCallback((requestId: string) => {
    setGameLaunchRequests((prev) => prev.filter((r) => r.request_id !== requestId));
  }, []);

  const clearPendingSplitContinuation = useCallback(() => {
    setPendingSplitContinuation(null);
  }, []);

  const sendDeployRolling = useCallback((binaryUrl: string) => {
    sendCommand("deploy_rolling", { binary_url: binaryUrl });
  }, [sendCommand]);

  return {
    connected,
    pods,
    latestTelemetry,
    recentLaps,
    billingTimers,
    billingWarnings,
    gameStates,
    pendingAuthTokens,
    assistanceRequests,
    dismissAssistance,
    gameLaunchRequests,
    dismissGameRequest,
    cameraFocus,
    activityLog,
    sendCommand,
    pendingSplitContinuation,
    clearPendingSplitContinuation,
    deployStates,
    sendDeployRolling,
    acServerInfo,
    multiplayerGroup,
  };
}
