"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import type { Pod, TelemetryFrame, Lap, BillingSession } from "@/lib/api";

const WS_URL = process.env.NEXT_PUBLIC_WS_URL || "ws://localhost:8080/ws/dashboard";

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
  const [connected, setConnected] = useState(false);
  const [pods, setPods] = useState<Map<string, Pod>>(new Map());
  const [latestTelemetry, setLatestTelemetry] = useState<TelemetryFrame | null>(null);
  const [recentLaps, setRecentLaps] = useState<Lap[]>([]);
  const [billingTimers, setBillingTimers] = useState<Map<string, BillingSession>>(new Map());
  const [billingWarnings, setBillingWarnings] = useState<BillingWarning[]>([]);

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
        }
      } catch (e) {
        console.warn("[RaceControl] Parse error:", e);
      }
    };

    socket.onclose = () => {
      setConnected(false);
      console.log("[RaceControl] Disconnected, retrying in 3s...");
      setTimeout(connect, 3000);
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
    sendCommand,
  };
}
