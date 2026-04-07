"use client";

import { useEffect, useRef, useState, useCallback } from "react";

// ─── Constants ─────────────────────────────────────────────────────────────────

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";
const WS_BASE_RAW = process.env.NEXT_PUBLIC_WS_URL || "ws://localhost:8080/ws/dashboard";
// Derive the spectator WS URL from the dashboard WS URL base
const WS_SPECTATOR = WS_BASE_RAW.replace("/ws/dashboard", "/ws/spectator");

const CANVAS_PADDING = 40;
const CAR_DOT_RADIUS = 10;
const TRACK_LINE_WIDTH = 4;
const TRACK_COLOR = "#444444";
const TRACK_GLOW_COLOR = "#555555";
const BACKGROUND_COLOR = "#0A0A0A";
const TEXT_COLOR = "#FFFFFF";
const MUTED_COLOR = "#888888";
const RACING_RED = "#E10600";

// Pod colors — each pod gets a distinct color for easy identification
const POD_COLORS: Record<number, string> = {
  1: "#E10600", // Racing Red
  2: "#00BFFF", // Deep Sky Blue
  3: "#00FF7F", // Spring Green
  4: "#FFD700", // Gold
  5: "#FF69B4", // Hot Pink
  6: "#9370DB", // Medium Purple
  7: "#FF8C00", // Dark Orange
  8: "#00CED1", // Dark Turquoise
};

function getPodColor(podNumber: number): string {
  return POD_COLORS[podNumber] || "#FFFFFF";
}

// ─── Types ─────────────────────────────────────────────────────────────────────

interface TrackOutline {
  track_id: string;
  config: string;
  points: [number, number][];
  original_point_count: number;
}

interface CarPosition {
  pod_id: string;
  pod_number: number;
  driver_name: string;
  normalized_position: number;
  lap: number;
  speed_kmh: number;
  track: string;
  car: string;
  is_in_pit: boolean;
}

interface SpectatorMessage {
  type: "car_positions" | "track_changed";
  positions?: CarPosition[];
  track?: string;
  track_id?: string;
  has_outline?: boolean;
  timestamp?: string;
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

function prettyTrackName(raw: string): string {
  if (!raw) return "Unknown Track";
  return raw
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .trim();
}

function prettyCarName(raw: string): string {
  if (!raw) return "";
  const segment = raw.split(/[/\\]/).pop() || raw;
  return segment
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .trim();
}

function formatSpeed(kmh: number): string {
  return `${Math.round(kmh)} km/h`;
}

/** Interpolate a point along a polyline given a normalized position [0, 1]. */
function interpolateOnPolyline(
  points: [number, number][],
  normalizedPos: number
): { x: number; y: number } {
  if (points.length === 0) return { x: 0.5, y: 0.5 };
  if (points.length === 1) return { x: points[0][0], y: points[0][1] };

  // Clamp
  const t = Math.max(0, Math.min(1, normalizedPos));

  // The normalized position maps linearly to the point index
  const totalSegments = points.length; // Track is a closed loop
  const exactIndex = t * totalSegments;
  const idx = Math.floor(exactIndex) % points.length;
  const frac = exactIndex - Math.floor(exactIndex);
  const nextIdx = (idx + 1) % points.length;

  const x = points[idx][0] + (points[nextIdx][0] - points[idx][0]) * frac;
  const y = points[idx][1] + (points[nextIdx][1] - points[idx][1]) * frac;

  return { x, y };
}

// ─── Main Component ────────────────────────────────────────────────────────────

export default function SpectatorCircuitPage() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const animFrameRef = useRef<number>(0);

  const [connected, setConnected] = useState(false);
  const [currentTrack, setCurrentTrack] = useState<string>("");
  const [trackOutline, setTrackOutline] = useState<TrackOutline | null>(null);
  const [positions, setPositions] = useState<CarPosition[]>([]);
  const [lastUpdate, setLastUpdate] = useState<string>("");
  const [error, setError] = useState<string>("");

  // Fetch track outline from REST API
  const fetchTrackOutline = useCallback(async (trackId: string) => {
    try {
      const res = await fetch(`${API_BASE}/api/v1/spectator/track/${encodeURIComponent(trackId)}`);
      if (res.ok) {
        const data: TrackOutline = await res.json();
        setTrackOutline(data);
        setError("");
      } else {
        // Track outline not available — that's okay, we can still show dots
        setTrackOutline(null);
      }
    } catch (e) {
      console.warn("[Spectator] Failed to fetch track outline:", e);
      setTrackOutline(null);
    }
  }, []);

  // WebSocket connection
  const connectWs = useCallback(() => {
    if (typeof window === "undefined") return;
    if (wsRef.current?.readyState === WebSocket.OPEN) return;

    const socket = new WebSocket(WS_SPECTATOR);

    socket.onopen = () => {
      setConnected(true);
      setError("");
      console.log("[Spectator] WS connected");
    };

    socket.onmessage = (event) => {
      try {
        const msg: SpectatorMessage = JSON.parse(event.data);

        if (msg.type === "car_positions" && msg.positions) {
          setPositions(msg.positions);
          setLastUpdate(msg.timestamp || new Date().toISOString());

          // Auto-detect track from positions
          const track = msg.track || msg.positions[0]?.track || "";
          if (track && track !== currentTrack) {
            setCurrentTrack(track);
            fetchTrackOutline(track);
          }
        } else if (msg.type === "track_changed" && msg.track_id) {
          setCurrentTrack(msg.track_id);
          fetchTrackOutline(msg.track_id);
        }
      } catch (e) {
        console.warn("[Spectator] Failed to parse message:", e);
      }
    };

    socket.onclose = () => {
      setConnected(false);
      console.log("[Spectator] WS disconnected, reconnecting in 3s...");
      reconnectTimerRef.current = setTimeout(connectWs, 3000);
    };

    socket.onerror = () => {
      setError("Connection error");
    };

    wsRef.current = socket;
  }, [currentTrack, fetchTrackOutline]);

  // Connect on mount
  useEffect(() => {
    connectWs();
    return () => {
      if (reconnectTimerRef.current) clearTimeout(reconnectTimerRef.current);
      wsRef.current?.close();
    };
  }, [connectWs]);

  // Canvas rendering loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const render = () => {
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      ctx.scale(dpr, dpr);

      const w = rect.width;
      const h = rect.height;

      // Clear background
      ctx.fillStyle = BACKGROUND_COLOR;
      ctx.fillRect(0, 0, w, h);

      // Drawing area (leave space for the sidebar)
      const sidebarWidth = 320;
      const drawW = w - sidebarWidth - CANVAS_PADDING * 2;
      const drawH = h - CANVAS_PADDING * 2;
      const drawX = CANVAS_PADDING;
      const drawY = CANVAS_PADDING;

      // Use the smaller dimension to maintain aspect ratio
      const drawSize = Math.min(drawW, drawH);
      const offsetX = drawX + (drawW - drawSize) / 2;
      const offsetY = drawY + (drawH - drawSize) / 2;

      // Transform normalized [0, 1] coordinates to canvas coordinates
      const toCanvas = (nx: number, ny: number): { x: number; y: number } => ({
        x: offsetX + nx * drawSize,
        y: offsetY + ny * drawSize,
      });

      // Draw track outline
      if (trackOutline && trackOutline.points.length > 2) {
        const pts = trackOutline.points;

        // Track glow (wider, softer line)
        ctx.beginPath();
        const start = toCanvas(pts[0][0], pts[0][1]);
        ctx.moveTo(start.x, start.y);
        for (let i = 1; i < pts.length; i++) {
          const p = toCanvas(pts[i][0], pts[i][1]);
          ctx.lineTo(p.x, p.y);
        }
        ctx.closePath();
        ctx.strokeStyle = TRACK_GLOW_COLOR;
        ctx.lineWidth = TRACK_LINE_WIDTH + 4;
        ctx.lineCap = "round";
        ctx.lineJoin = "round";
        ctx.globalAlpha = 0.3;
        ctx.stroke();
        ctx.globalAlpha = 1.0;

        // Track main line
        ctx.beginPath();
        ctx.moveTo(start.x, start.y);
        for (let i = 1; i < pts.length; i++) {
          const p = toCanvas(pts[i][0], pts[i][1]);
          ctx.lineTo(p.x, p.y);
        }
        ctx.closePath();
        ctx.strokeStyle = TRACK_COLOR;
        ctx.lineWidth = TRACK_LINE_WIDTH;
        ctx.stroke();

        // Start/finish line marker
        const sfPos = toCanvas(pts[0][0], pts[0][1]);
        ctx.beginPath();
        ctx.arc(sfPos.x, sfPos.y, 6, 0, Math.PI * 2);
        ctx.fillStyle = "#FFFFFF";
        ctx.globalAlpha = 0.5;
        ctx.fill();
        ctx.globalAlpha = 1.0;
      }

      // Draw car dots
      const activePositions = positions.filter((p) => !p.is_in_pit);
      const pitPositions = positions.filter((p) => p.is_in_pit);

      // Draw pit cars as smaller, dimmer dots (off to the side)
      pitPositions.forEach((car) => {
        const color = getPodColor(car.pod_number);
        // Place pit cars in a column on the left
        const pitIdx = pitPositions.indexOf(car);
        const pitX = offsetX - 30;
        const pitY = offsetY + 60 + pitIdx * 30;

        ctx.beginPath();
        ctx.arc(pitX, pitY, 5, 0, Math.PI * 2);
        ctx.fillStyle = color;
        ctx.globalAlpha = 0.4;
        ctx.fill();
        ctx.globalAlpha = 1.0;

        // Label
        ctx.font = "10px monospace";
        ctx.fillStyle = MUTED_COLOR;
        ctx.textAlign = "right";
        ctx.fillText(`P${car.pod_number} PIT`, pitX - 10, pitY + 4);
      });

      // Draw active cars on track
      activePositions.forEach((car) => {
        const color = getPodColor(car.pod_number);
        let cx: number, cy: number;

        if (trackOutline && trackOutline.points.length > 2) {
          const interp = interpolateOnPolyline(trackOutline.points, car.normalized_position);
          const canvasPos = toCanvas(interp.x, interp.y);
          cx = canvasPos.x;
          cy = canvasPos.y;
        } else {
          // No track outline — arrange cars in a circle
          const angle = car.normalized_position * Math.PI * 2 - Math.PI / 2;
          const radius = drawSize * 0.35;
          const centerX = offsetX + drawSize / 2;
          const centerY = offsetY + drawSize / 2;
          cx = centerX + Math.cos(angle) * radius;
          cy = centerY + Math.sin(angle) * radius;
        }

        // Glow
        ctx.beginPath();
        ctx.arc(cx, cy, CAR_DOT_RADIUS + 4, 0, Math.PI * 2);
        ctx.fillStyle = color;
        ctx.globalAlpha = 0.2;
        ctx.fill();
        ctx.globalAlpha = 1.0;

        // Dot
        ctx.beginPath();
        ctx.arc(cx, cy, CAR_DOT_RADIUS, 0, Math.PI * 2);
        ctx.fillStyle = color;
        ctx.fill();

        // Pod number inside dot
        ctx.font = "bold 11px sans-serif";
        ctx.fillStyle = "#FFFFFF";
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(String(car.pod_number), cx, cy);

        // Driver name label
        ctx.font = "11px sans-serif";
        ctx.fillStyle = color;
        ctx.textAlign = "left";
        ctx.textBaseline = "middle";
        ctx.fillText(car.driver_name || `Pod ${car.pod_number}`, cx + CAR_DOT_RADIUS + 6, cy);
      });

      // ─── Sidebar ───────────────────────────────────────────────────────
      const sbX = w - sidebarWidth;

      // Sidebar background
      ctx.fillStyle = "#111111";
      ctx.fillRect(sbX, 0, sidebarWidth, h);
      ctx.strokeStyle = "#222222";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(sbX, 0);
      ctx.lineTo(sbX, h);
      ctx.stroke();

      // Title
      ctx.font = "bold 18px sans-serif";
      ctx.fillStyle = RACING_RED;
      ctx.textAlign = "left";
      ctx.textBaseline = "top";
      ctx.fillText("RACING POINT", sbX + 16, 20);

      ctx.font = "12px sans-serif";
      ctx.fillStyle = MUTED_COLOR;
      ctx.fillText("LIVE CIRCUIT", sbX + 16, 44);

      // Track name
      if (currentTrack) {
        ctx.font = "bold 16px sans-serif";
        ctx.fillStyle = TEXT_COLOR;
        ctx.fillText(prettyTrackName(currentTrack), sbX + 16, 72);
      }

      // Connection status
      const statusY = 100;
      ctx.beginPath();
      ctx.arc(sbX + 24, statusY, 5, 0, Math.PI * 2);
      ctx.fillStyle = connected ? "#00FF7F" : "#FF4444";
      ctx.fill();
      ctx.font = "11px sans-serif";
      ctx.fillStyle = MUTED_COLOR;
      ctx.textAlign = "left";
      ctx.fillText(connected ? "LIVE" : "DISCONNECTED", sbX + 36, statusY + 4);

      // Driver list
      const listY = 130;
      ctx.font = "bold 12px sans-serif";
      ctx.fillStyle = MUTED_COLOR;
      ctx.fillText("DRIVERS", sbX + 16, listY);

      // Sort: by lap count desc, then position
      const sorted = [...positions].sort((a, b) => {
        if (b.lap !== a.lap) return b.lap - a.lap;
        return a.pod_number - b.pod_number;
      });

      sorted.forEach((car, idx) => {
        const y = listY + 24 + idx * 48;
        if (y > h - 40) return; // Don't overflow

        const color = getPodColor(car.pod_number);

        // Color bar
        ctx.fillStyle = color;
        ctx.fillRect(sbX + 16, y, 3, 36);

        // Pod number
        ctx.font = "bold 14px sans-serif";
        ctx.fillStyle = color;
        ctx.textAlign = "left";
        ctx.fillText(`P${car.pod_number}`, sbX + 28, y + 12);

        // Driver name
        ctx.font = "13px sans-serif";
        ctx.fillStyle = TEXT_COLOR;
        ctx.fillText(car.driver_name || "---", sbX + 60, y + 12);

        // Car name
        ctx.font = "10px sans-serif";
        ctx.fillStyle = MUTED_COLOR;
        ctx.fillText(prettyCarName(car.car), sbX + 60, y + 28);

        // Lap count
        ctx.font = "bold 12px monospace";
        ctx.fillStyle = TEXT_COLOR;
        ctx.textAlign = "right";
        ctx.fillText(`L${car.lap}`, sbX + sidebarWidth - 16, y + 12);

        // Speed
        ctx.font = "10px monospace";
        ctx.fillStyle = car.is_in_pit ? "#FF8C00" : MUTED_COLOR;
        ctx.fillText(
          car.is_in_pit ? "PIT" : formatSpeed(car.speed_kmh),
          sbX + sidebarWidth - 16,
          y + 28
        );
      });

      // No drivers message
      if (positions.length === 0) {
        ctx.font = "14px sans-serif";
        ctx.fillStyle = MUTED_COLOR;
        ctx.textAlign = "center";
        const centerX = offsetX + drawSize / 2;
        const centerY = offsetY + drawSize / 2;
        ctx.fillText("Waiting for drivers...", centerX, centerY);
        ctx.font = "11px sans-serif";
        ctx.fillText("Cars will appear when a session is active", centerX, centerY + 24);
      }

      // Footer timestamp
      if (lastUpdate) {
        ctx.font = "10px monospace";
        ctx.fillStyle = "#333333";
        ctx.textAlign = "right";
        ctx.fillText(lastUpdate, w - 16, h - 8);
      }

      animFrameRef.current = requestAnimationFrame(render);
    };

    animFrameRef.current = requestAnimationFrame(render);

    return () => {
      cancelAnimationFrame(animFrameRef.current);
    };
  }, [trackOutline, positions, currentTrack, connected, lastUpdate]);

  // Handle window resize
  useEffect(() => {
    const handleResize = () => {
      // Canvas will re-render on next animation frame
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  return (
    <div className="fixed inset-0 bg-[#0A0A0A] overflow-hidden cursor-none">
      <canvas
        ref={canvasRef}
        className="w-full h-full"
        style={{ display: "block" }}
      />
      {error && (
        <div className="absolute top-4 left-4 bg-red-900/80 text-red-200 px-4 py-2 rounded text-sm">
          {error}
        </div>
      )}
    </div>
  );
}
