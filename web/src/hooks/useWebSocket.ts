"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import type { Pod, TelemetryFrame, Lap, BillingSession, GameLaunchInfo, AiDebugSuggestion, AcServerInfo, AcPresetSummary, AcLanSessionConfig, AuthTokenInfo, FeatureFlagRow } from "@/lib/api";

const WS_BASE = process.env.NEXT_PUBLIC_WS_URL || "ws://localhost:8080/ws/dashboard";
const WS_TOKEN = process.env.NEXT_PUBLIC_WS_TOKEN || "";
const WS_URL = WS_TOKEN ? `${WS_BASE}?token=${WS_TOKEN}` : WS_BASE;

interface DashboardEvent {
  event: string;
  data: unknown;
}

export interface BillingWarning {
  sessionId: string;
  podId: string;
  remaining: number;
  timestamp: number;
}

export function useWebSocket() {
  const ws = useRef<WebSocket | null>(null);
  const reconnectAttemptRef = useRef(0);
  const [connected, setConnected] = useState(false);
  const [pods, setPods] = useState<Map<string, Pod>>(new Map());
  const [latestTelemetry, setLatestTelemetry] = useState<TelemetryFrame | null>(null);
  const [recentLaps, setRecentLaps] = useState<Lap[]>([]);
  const [billingTimers, setBillingTimers] = useState<Map<string, BillingSession>>(new Map());
  const [billingWarnings, setBillingWarnings] = useState<BillingWarning[]>([]);
  const [gameStates, setGameStates] = useState<Map<string, GameLaunchInfo>>(new Map());
  const [aiDebugSuggestions, setAiDebugSuggestions] = useState<AiDebugSuggestion[]>([]);
  const [acServerInfo, setAcServerInfo] = useState<AcServerInfo | null>(null);
  const [acPresets, setAcPresets] = useState<AcPresetSummary[]>([]);
  const [acLoadedConfig, setAcLoadedConfig] = useState<{ presetId: string; config: AcLanSessionConfig } | null>(null);
  const [pendingAuthTokens, setPendingAuthTokens] = useState<Map<string, AuthTokenInfo>>(new Map());
  const [featureFlags, setFeatureFlags] = useState<FeatureFlagRow[]>([]);

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

    const socket = new WebSocket(WS_URL);

    socket.onopen = () => {
      reconnectAttemptRef.current = 0;
      setConnected(true);
      console.log("[RaceControl] Connected to server");
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
            setLatestTelemetry(msg.data as TelemetryFrame);
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
            setBillingTimers((prev) => {
              const next = new Map(prev);
              if (
                session.status === "completed" ||
                session.status === "cancelled" ||
                session.status === "ended_early"
              ) {
                next.delete(session.pod_id);
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
            // Auto-clear after 10 seconds
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
          case "ai_debug_suggestion": {
            const suggestion = msg.data as AiDebugSuggestion;
            setAiDebugSuggestions((prev) => [suggestion, ...prev].slice(0, 20));
            break;
          }
          case "ac_server_update": {
            const info = msg.data as AcServerInfo;
            if (info.status === "stopped") {
              setAcServerInfo(null);
            } else {
              setAcServerInfo(info);
            }
            break;
          }
          case "ac_preset_list": {
            setAcPresets(msg.data as AcPresetSummary[]);
            break;
          }
          case "ac_preset_loaded": {
            const d = msg.data as { preset_id: string; config: AcLanSessionConfig };
            setAcLoadedConfig({ presetId: d.preset_id, config: d.config });
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
            const d = msg.data as { token_id: string; pod_id: string; billing_session_id: string };
            setPendingAuthTokens((prev) => {
              const next = new Map(prev);
              next.delete(d.pod_id);
              return next;
            });
            break;
          }
          case "auth_token_cleared": {
            const d = msg.data as { token_id: string; pod_id: string; reason: string };
            setPendingAuthTokens((prev) => {
              const next = new Map(prev);
              next.delete(d.pod_id);
              return next;
            });
            break;
          }
          case "flag_sync": {
            const flags = msg.data as FeatureFlagRow[];
            setFeatureFlags(flags);
            break;
          }
          case "flag_updated": {
            const flag = msg.data as FeatureFlagRow;
            setFeatureFlags((prev) => {
              const idx = prev.findIndex((f) => f.name === flag.name);
              if (idx >= 0) {
                const next = [...prev];
                next[idx] = flag;
                return next;
              }
              return [...prev, flag];
            });
            break;
          }
        }
      } catch (e) {
        console.warn("[RaceControl] Parse error:", e);
      }
    };

    socket.onclose = () => {
      setConnected(false);
      const attempt = reconnectAttemptRef.current;
      const baseDelay = Math.min(1000 * Math.pow(2, attempt), 30_000);
      const jitter = Math.random() * baseDelay * 0.3;
      const delay = Math.round(baseDelay + jitter);
      reconnectAttemptRef.current = attempt + 1;
      console.log(`[RaceControl] Disconnected, retrying in ${delay}ms (attempt ${attempt + 1})...`);
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
    };
  }, [connect]);

  return {
    connected,
    pods: Array.from(pods.values()),
    latestTelemetry,
    recentLaps,
    billingTimers,
    billingWarnings,
    gameStates,
    aiDebugSuggestions,
    acServerInfo,
    acPresets,
    acLoadedConfig,
    pendingAuthTokens,
    featureFlags,
    sendCommand,
  };
}
