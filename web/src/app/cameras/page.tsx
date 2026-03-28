"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import DashboardLayout from "@/components/DashboardLayout";

// ── Constants ──────────────────────────────────────────────────────────────────
const SENTRY_BASE = "http://192.168.31.27:8096";
const SENTRY_WS = "ws://192.168.31.27:8096";
const ZONE_ORDER = ["entrance", "reception", "pods", "other"] as const;

// ── TypeScript interfaces ──────────────────────────────────────────────────────
interface CameraInfo {
  name: string;
  display_name: string;
  display_order: number;
  zone: string;
  nvr_channel: number;
  status: string;
  role: string;
  stream_url: string;
}

interface LayoutState {
  grid_mode: string;
  camera_order: number[];
  zone_filter: string | null;
}

type GridMode = "1x1" | "2x2" | "3x3" | "4x4";

type StreamStatus = "connecting" | "connected" | "failed" | "disconnected" | "closed";

// Check if browser supports H.265 via WebCodecs (Chrome 94+, Edge 94+, Safari 16.4+)
function supportsWebCodecs(): boolean {
  return typeof globalThis.VideoDecoder !== "undefined";
}

// ── Animation keyframes injected once ──────────────────────────────────────────
const STREAM_STYLE = `
@keyframes fs-fade-in {
  from { opacity: 0; }
  to   { opacity: 1; }
}
@keyframes rtc-pulse {
  0%, 100% { opacity: 1; }
  50%       { opacity: 0.4; }
}
`;

// ── Grid column class map ──────────────────────────────────────────────────────
const GRID_COLS: Record<GridMode, string> = {
  "1x1": "grid-cols-1",
  "2x2": "grid-cols-2",
  "3x3": "grid-cols-3",
  "4x4": "grid-cols-4",
};

// ── Status dot colour ──────────────────────────────────────────────────────────
function statusDotClass(status: string): string {
  if (status === "connected") return "bg-green-500";
  if (status === "reconnecting") return "bg-yellow-400";
  return "bg-red-500";
}

function isOffline(status: string): boolean {
  return status === "offline" || status === "disconnected";
}

// ── Live stream connection — on-demand NVR streaming via rc-sentry-ai ──────────
// Primary: WebSocket + WebCodecs VideoDecoder (H.265 native, Chrome/Edge/Safari)
// Fallback: MJPEG proxy from NVR (Firefox, older browsers)

interface LiveStreamConnection {
  ws: WebSocket | null;
  decoder: VideoDecoder | null;
  cleanup: () => void;
}

function connectLiveStream(
  channel: number,
  canvas: HTMLCanvasElement,
  onStatus: (state: string) => void,
): LiveStreamConnection {
  const ws = new WebSocket(`${SENTRY_WS}/api/v1/stream/ws/${channel}?subtype=0`);
  ws.binaryType = "arraybuffer";

  const ctx = canvas.getContext("2d");
  let decoder: VideoDecoder | null = null;
  let configured = false;

  if (supportsWebCodecs()) {
    decoder = new VideoDecoder({
      output: (frame: VideoFrame) => {
        if (ctx) {
          if (canvas.width !== frame.displayWidth || canvas.height !== frame.displayHeight) {
            canvas.width = frame.displayWidth;
            canvas.height = frame.displayHeight;
          }
          ctx.drawImage(frame, 0, 0);
        }
        frame.close();
        if (!configured) {
          configured = true;
          onStatus("connected");
        }
      },
      error: (e: DOMException) => {
        console.error("VideoDecoder error:", e);
        onStatus("failed");
      },
    });
  }

  ws.onmessage = (ev: MessageEvent) => {
    if (typeof ev.data === "string") {
      // Init message from server: {"type":"init","codec":"hev1.1.6.L123.B0","width":2560,"height":1440}
      try {
        const config = JSON.parse(ev.data) as { type: string; codec: string; width: number; height: number; msg?: string };
        if (config.type === "init" && decoder) {
          decoder.configure({
            codec: config.codec,
            codedWidth: config.width,
            codedHeight: config.height,
            optimizeForLatency: true,
          });
          onStatus("connecting");
        } else if (config.type === "error") {
          console.error("Stream error:", config.msg);
          onStatus("failed");
        }
      } catch {
        // ignore parse errors
      }
    } else if (decoder && ev.data instanceof ArrayBuffer) {
      // Binary frame: [8-byte LE timestamp µs][1-byte flags][H.265 Annex B data]
      const buf = ev.data;
      if (buf.byteLength < 10) return;
      const view = new DataView(buf);
      const timestamp = Number(view.getBigUint64(0, true));
      const flags = view.getUint8(8);
      const data = new Uint8Array(buf, 9);

      try {
        decoder.decode(
          new EncodedVideoChunk({
            type: flags & 0x01 ? "key" : "delta",
            timestamp: timestamp,
            data: data,
          }),
        );
      } catch (e) {
        console.warn("decode error:", e);
      }
    }
  };

  ws.onerror = () => onStatus("failed");
  ws.onclose = () => onStatus("closed");

  const cleanup = () => {
    if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
      ws.close();
    }
    if (decoder && decoder.state !== "closed") {
      try { decoder.close(); } catch { /* already closed */ }
    }
  };

  return { ws, decoder, cleanup };
}

// ── Main component ─────────────────────────────────────────────────────────────
export default function CamerasPage() {
  // ── State ────────────────────────────────────────────────────────────────────
  const [cameras, setCameras] = useState<CameraInfo[]>([]);
  const [gridMode, setGridMode] = useState<GridMode>("3x3");
  const [refreshRate, setRefreshRate] = useState<number>(2000);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [collapsedZones, setCollapsedZones] = useState<Record<string, boolean>>({});
  const [fullscreenCamera, setFullscreenCamera] = useState<CameraInfo | null>(null);
  const [streamStatus, setStreamStatus] = useState<StreamStatus>("connecting");
  const [dragOverChannel, setDragOverChannel] = useState<number | null>(null);
  const [statusText, setStatusText] = useState<string>("Loading...");
  const [draggingChannel, setDraggingChannel] = useState<number | null>(null);
  const [showFallback, setShowFallback] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [useMjpegFallback, setUseMjpegFallback] = useState(!supportsWebCodecs());

  // ── Refs ─────────────────────────────────────────────────────────────────────
  const liveStreamRef = useRef<LiveStreamConnection | null>(null);
  const controlsHideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const mjpegImgRef = useRef<HTMLImageElement | null>(null);
  const refreshTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const imgRefs = useRef<Record<number, HTMLImageElement>>({});
  const dragSrcChannelRef = useRef<number | null>(null);
  const camerasRef = useRef<CameraInfo[]>([]);

  // Keep camerasRef in sync with cameras state (for interval callbacks)
  useEffect(() => {
    camerasRef.current = cameras;
  }, [cameras]);

  // ── Inject animation styles ──────────────────────────────────────────────────
  useEffect(() => {
    const existing = document.getElementById("cameras-page-styles");
    if (!existing) {
      const style = document.createElement("style");
      style.id = "cameras-page-styles";
      style.textContent = STREAM_STYLE;
      document.head.appendChild(style);
    }
    return () => {
      const el = document.getElementById("cameras-page-styles");
      if (el) el.remove();
    };
  }, []);

  // ── teardownStream ────────────────────────────────────────────────────────────
  const teardownStream = useCallback(() => {
    if (liveStreamRef.current) {
      liveStreamRef.current.cleanup();
      liveStreamRef.current = null;
    }
  }, []);

  // ── Layout persistence ────────────────────────────────────────────────────────
  const saveLayout = useCallback((cams: CameraInfo[], mode: GridMode) => {
    const body: LayoutState = {
      grid_mode: mode,
      camera_order: cams.map((c) => c.nvr_channel).filter(Boolean),
      zone_filter: null,
    };
    fetch(`${SENTRY_BASE}/api/v1/cameras/layout`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    }).catch((err) => console.warn("Layout save failed:", err));
  }, []);

  // ── Snapshot refresh loop ────────────────────────────────────────────────────
  const startRefreshLoop = useCallback((rate: number) => {
    if (refreshTimerRef.current) clearInterval(refreshTimerRef.current);

    const doRefresh = () => {
      const cams = camerasRef.current;
      let ok = 0;
      let fail = 0;
      let total = 0;

      cams.forEach((camera) => {
        const ch = camera.nvr_channel;
        if (!ch) return;
        total++;

        const preload = new Image();
        const url = `${SENTRY_BASE}/api/v1/cameras/nvr/${ch}/snapshot?t=${Date.now()}`;

        preload.onload = () => {
          const imgEl = imgRefs.current[ch];
          if (imgEl) imgEl.src = preload.src;
          ok++;
          if (ok + fail === total) {
            setStatusText(`${ok}/${cams.length} online`);
          }
        };

        preload.onerror = () => {
          fail++;
          if (ok + fail === total) {
            setStatusText(`${ok}/${cams.length} online`);
          }
        };

        preload.src = url;
      });

      if (total === 0) setStatusText(`0/${cams.length} online`);
    };

    doRefresh();
    refreshTimerRef.current = setInterval(doRefresh, rate);
  }, []);

  // ── Fullscreen controls auto-hide ─────────────────────────────────────────────
  const resetControlsTimer = useCallback(() => {
    setControlsVisible(true);
    if (controlsHideTimerRef.current) clearTimeout(controlsHideTimerRef.current);
    controlsHideTimerRef.current = setTimeout(() => {
      setControlsVisible(false);
    }, 3000);
  }, []);

  // ── Open fullscreen ───────────────────────────────────────────────────────────
  const openFullscreen = useCallback(
    (camera: CameraInfo) => {
      teardownStream();
      setFullscreenCamera(camera);
      setStreamStatus("connecting");
      setShowFallback(false);

      // Determine streaming mode
      const useWebCodecs = supportsWebCodecs() && !useMjpegFallback;

      if (useWebCodecs) {
        // Use WebSocket + VideoDecoder for H.265 native quality (4MP)
        // Canvas ref will be available after render — connect in a microtask
        setTimeout(() => {
          if (canvasRef.current && camera.nvr_channel) {
            const conn = connectLiveStream(
              camera.nvr_channel,
              canvasRef.current,
              (state) => {
                setStreamStatus(state as StreamStatus);
                if (state === "failed") {
                  // Fall back to MJPEG on WebCodecs failure
                  setUseMjpegFallback(true);
                  setShowFallback(true);
                }
              },
            );
            liveStreamRef.current = conn;
          }
        }, 0);
      } else {
        // MJPEG fallback — set img src to NVR MJPEG proxy
        setTimeout(() => {
          if (mjpegImgRef.current && camera.nvr_channel) {
            mjpegImgRef.current.src = `${SENTRY_BASE}/api/v1/stream/mjpeg/${camera.nvr_channel}?subtype=1`;
            setStreamStatus("connected");
          }
        }, 0);
      }

      resetControlsTimer();
    },
    [teardownStream, resetControlsTimer, useMjpegFallback],
  );

  // ── Close fullscreen ──────────────────────────────────────────────────────────
  const closeFullscreen = useCallback(() => {
    teardownStream();
    setFullscreenCamera(null);
    setStreamStatus("connecting");
    setShowFallback(false);
    setControlsVisible(true);
    if (controlsHideTimerRef.current) clearTimeout(controlsHideTimerRef.current);
    // Clear MJPEG img src to stop the stream
    if (mjpegImgRef.current) {
      mjpegImgRef.current.src = "";
    }
  }, [teardownStream]);

  // ── Initial data fetch ────────────────────────────────────────────────────────
  useEffect(() => {
    let cancelled = false;

    const init = async () => {
      setLoading(true);
      setError(null);

      try {
        // 1. Fetch camera list
        const camRes = await fetch(`${SENTRY_BASE}/api/v1/cameras`);
        if (!camRes.ok) throw new Error(`HTTP ${camRes.status}`);
        const camData = (await camRes.json()) as CameraInfo[];
        const sorted = camData.slice().sort((a, b) => (a.display_order || 0) - (b.display_order || 0));

        if (cancelled) return;
        setCameras(sorted);
        camerasRef.current = sorted;

        // 2. Fetch layout and apply
        let finalCameras = sorted;
        let finalMode: GridMode = "3x3";
        try {
          const layoutRes = await fetch(`${SENTRY_BASE}/api/v1/cameras/layout`);
          if (layoutRes.ok) {
            const layout = (await layoutRes.json()) as LayoutState;
            if (cancelled) return;

            if (layout.grid_mode && ["1x1", "2x2", "3x3", "4x4"].includes(layout.grid_mode)) {
              finalMode = layout.grid_mode as GridMode;
              setGridMode(finalMode);
            }

            if (layout.camera_order && layout.camera_order.length > 0) {
              const orderMap: Record<number, number> = {};
              layout.camera_order.forEach((ch, i) => { orderMap[ch] = i; });
              finalCameras = sorted.slice().sort((a, b) => {
                const aIdx = a.nvr_channel in orderMap ? orderMap[a.nvr_channel] : 9999;
                const bIdx = b.nvr_channel in orderMap ? orderMap[b.nvr_channel] : 9999;
                return aIdx - bIdx;
              });
              setCameras(finalCameras);
              camerasRef.current = finalCameras;
            }
          }
        } catch {
          // Layout fetch failed — proceed with defaults
        }

        setLoading(false);

        // 3. Start snapshot polling
        startRefreshLoop(refreshRate);
        void finalCameras; // suppress unused warning
        void finalMode;

      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Failed to fetch cameras");
          setLoading(false);
        }
      }
    };

    init();

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Restart refresh loop on rate change ───────────────────────────────────────
  useEffect(() => {
    if (!loading && cameras.length > 0) {
      startRefreshLoop(refreshRate);
    }
    return () => {
      if (refreshTimerRef.current) clearInterval(refreshTimerRef.current);
    };
  }, [refreshRate, loading, cameras.length, startRefreshLoop]);

  // ── Keyboard and lifecycle event listeners ────────────────────────────────────
  useEffect(() => {
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeFullscreen();
    };
    const onBeforeUnload = () => teardownStream();
    const onVisibilityChange = () => {
      if (document.hidden) teardownStream();
    };

    document.addEventListener("keydown", onKeydown);
    window.addEventListener("beforeunload", onBeforeUnload);
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      document.removeEventListener("keydown", onKeydown);
      window.removeEventListener("beforeunload", onBeforeUnload);
      document.removeEventListener("visibilitychange", onVisibilityChange);
      // Cleanup all timers and connections
      if (refreshTimerRef.current) clearInterval(refreshTimerRef.current);
      if (controlsHideTimerRef.current) clearTimeout(controlsHideTimerRef.current);
      teardownStream();
    };
  }, [closeFullscreen, teardownStream]);

  // ── Layout mode switching ─────────────────────────────────────────────────────
  const handleModeChange = useCallback(
    (mode: GridMode) => {
      setGridMode(mode);
      saveLayout(camerasRef.current, mode);
    },
    [saveLayout],
  );

  // ── Zone toggle ───────────────────────────────────────────────────────────────
  const toggleZone = useCallback((zone: string) => {
    setCollapsedZones((prev) => ({ ...prev, [zone]: !prev[zone] }));
  }, []);

  // ── Drag handlers ─────────────────────────────────────────────────────────────
  const handleDragStart = useCallback((e: React.DragEvent<HTMLDivElement>, channel: number) => {
    dragSrcChannelRef.current = channel;
    setDraggingChannel(channel);
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(channel));
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent<HTMLDivElement>, channel: number) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    setDragOverChannel(channel);
  }, []);

  const handleDragLeave = useCallback(() => {
    setDragOverChannel(null);
  }, []);

  const handleDragEnd = useCallback(() => {
    setDraggingChannel(null);
    setDragOverChannel(null);
    dragSrcChannelRef.current = null;
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent<HTMLDivElement>, targetChannel: number) => {
      e.preventDefault();
      setDragOverChannel(null);
      setDraggingChannel(null);

      const srcChannel = dragSrcChannelRef.current;
      if (srcChannel === null || srcChannel === targetChannel) return;

      setCameras((prev) => {
        const next = [...prev];
        const srcIdx = next.findIndex((c) => c.nvr_channel === srcChannel);
        const tgtIdx = next.findIndex((c) => c.nvr_channel === targetChannel);
        if (srcIdx === -1 || tgtIdx === -1) return prev;

        const [moved] = next.splice(srcIdx, 1);
        next.splice(tgtIdx, 0, moved);
        camerasRef.current = next;
        saveLayout(next, gridMode);
        return next;
      });

      dragSrcChannelRef.current = null;
    },
    [saveLayout, gridMode],
  );

  // Pre-warm removed — on-demand WS connects fast enough without pre-warming

  // ── Group cameras by zone ─────────────────────────────────────────────────────
  const groupedCameras = (() => {
    const groups: Record<string, CameraInfo[]> = {};
    ZONE_ORDER.forEach((z) => { groups[z] = []; });
    cameras.forEach((camera) => {
      const zone = camera.zone || "other";
      if (!groups[zone]) groups[zone] = [];
      groups[zone].push(camera);
    });
    return groups;
  })();

  // ── Stream status dot styling ────────────────────────────────────────────────
  const rtcDotStyle = (): React.CSSProperties => {
    if (streamStatus === "connecting") {
      return {
        width: 8, height: 8, borderRadius: "50%",
        background: "#ffc107",
        animation: "rtc-pulse 1s infinite",
        display: "inline-block",
        flexShrink: 0,
      };
    }
    if (streamStatus === "connected") {
      return { width: 8, height: 8, borderRadius: "50%", background: "#4caf50", display: "inline-block", flexShrink: 0 };
    }
    return { width: 8, height: 8, borderRadius: "50%", background: "#E10600", display: "inline-block", flexShrink: 0 };
  };

  // ── Retry handler ─────────────────────────────────────────────────────────────
  const handleRetry = useCallback(() => {
    window.location.reload();
  }, []);

  // ── Render ────────────────────────────────────────────────────────────────────
  return (
    <DashboardLayout>
      {/* Negative margins to cancel DashboardLayout p-6 — edge-to-edge grid */}
      <div className="-m-6 flex flex-col h-[calc(100vh-0px)] overflow-hidden">

        {/* ── Toolbar ─────────────────────────────────────────────────────── */}
        <div className="flex-none px-3 py-1 flex items-center gap-2 bg-rp-black border-b border-rp-border">
          <span className="text-[#E10600] font-bold text-sm uppercase tracking-wide mr-auto">
            RACING POINT
          </span>
          <span className="text-[0.7rem] text-[#999]">{statusText}</span>

          {/* Layout mode buttons */}
          {(["1x1", "2x2", "3x3", "4x4"] as GridMode[]).map((mode, i) => {
            const labels = ["1", "4", "9", "16"];
            const isActive = gridMode === mode;
            return (
              <button
                key={mode}
                onClick={() => handleModeChange(mode)}
                className={`w-7 h-7 rounded border text-[0.65rem] font-bold font-mono flex items-center justify-center transition-colors duration-150 ${
                  isActive
                    ? "bg-[#E10600] border-[#E10600] text-white"
                    : "bg-[#333] border-[#555] text-[#ccc] hover:bg-[#444]"
                }`}
              >
                {labels[i]}
              </button>
            );
          })}

          {/* Refresh rate selector */}
          <select
            value={refreshRate}
            onChange={(e) => setRefreshRate(Number(e.target.value))}
            className="bg-[#333] text-white border border-[#555] rounded px-1.5 py-0.5 text-[0.7rem]"
          >
            <option value={1000}>1 fps</option>
            <option value={2000}>0.5 fps</option>
            <option value={5000}>0.2 fps</option>
          </select>
        </div>

        {/* ── Main content area ────────────────────────────────────────────── */}
        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <div className="flex items-center justify-center h-full text-rp-grey">
              <p className="animate-pulse text-sm">Loading camera feeds...</p>
            </div>
          ) : error ? (
            <div className="flex flex-col items-center justify-center h-full gap-3">
              <p className="text-red-400 text-sm">{error}</p>
              <button
                onClick={handleRetry}
                className="px-4 py-2 rounded text-xs font-medium bg-rp-card text-rp-grey border border-rp-border hover:text-white transition-colors"
              >
                Retry
              </button>
            </div>
          ) : cameras.length === 0 ? (
            <div className="flex items-center justify-center h-full text-rp-grey">
              <p className="text-sm">No cameras configured.</p>
            </div>
          ) : (
            /* ── Camera grid ─────────────────────────────────────────────── */
            <div
              className={`grid gap-0.5 p-0.5 transition-all duration-300 ease-in-out ${GRID_COLS[gridMode]}`}
            >
              {ZONE_ORDER.map((zone) => {
                const zoneCams = groupedCameras[zone];
                if (!zoneCams || zoneCams.length === 0) return null;
                const isCollapsed = !!collapsedZones[zone];

                return [
                  /* Zone header */
                  <div
                    key={`zone-${zone}`}
                    className="col-span-full px-2.5 py-1 flex items-center gap-1.5 bg-rp-black rounded cursor-pointer select-none hover:text-[#ccc] text-[#999] uppercase text-[0.65rem] font-bold tracking-widest"
                    onClick={() => toggleZone(zone)}
                  >
                    <span
                      className="text-[0.5rem] transition-transform duration-200"
                      style={{ display: "inline-block", transform: isCollapsed ? "rotate(-90deg)" : "rotate(0deg)" }}
                    >
                      ▼
                    </span>
                    <span>
                      {zone.toUpperCase()} ({zoneCams.length})
                    </span>
                  </div>,

                  /* Camera tiles */
                  ...zoneCams.map((camera) => {
                    const ch = camera.nvr_channel;
                    const offline = isOffline(camera.status);
                    const isDragging = draggingChannel === ch;
                    const isDragOver = dragOverChannel === ch;

                    return (
                      <div
                        key={camera.name}
                        draggable
                        onDragStart={(e) => handleDragStart(e, ch)}
                        onDragOver={(e) => handleDragOver(e, ch)}
                        onDragLeave={handleDragLeave}
                        onDragEnd={handleDragEnd}
                        onDrop={(e) => handleDrop(e, ch)}
                        className={`relative aspect-video bg-rp-card rounded overflow-hidden cursor-grab ${
                          offline ? "opacity-40" : ""
                        } ${isDragging ? "opacity-60" : ""}`}
                        style={{
                          display: isCollapsed ? "none" : undefined,
                          outline: isDragOver ? "2px dashed #E10600" : undefined,
                          outlineOffset: "-2px",
                        }}
                      >
                        {/* Label bar */}
                        <div className="absolute top-0 inset-x-0 px-2 py-0.5 flex justify-between items-center bg-black/65 pointer-events-none z-10">
                          <span className="text-[0.6rem] uppercase tracking-wide text-white truncate">
                            {camera.display_name}
                          </span>
                          <span
                            className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${statusDotClass(camera.status)}`}
                          />
                        </div>

                        {/* Snapshot image */}
                        {/* eslint-disable-next-line @next/next/no-img-element */}
                        <img
                          ref={(el) => {
                            if (el) imgRefs.current[ch] = el;
                          }}
                          src={`${SENTRY_BASE}/api/v1/cameras/nvr/${ch}/snapshot?t=0`}
                          alt={camera.display_name}
                          onClick={() => !offline && openFullscreen(camera)}
                          className="w-full h-full object-cover bg-black cursor-pointer"
                        />

                        {/* Offline overlay */}
                        {offline && (
                          <span
                            className="absolute inset-0 flex items-center justify-center text-[0.75rem] font-bold tracking-widest pointer-events-none z-20"
                            style={{ color: "#E10600" }}
                          >
                            OFFLINE
                          </span>
                        )}
                      </div>
                    );
                  }),
                ];
              })}
            </div>
          )}
        </div>
      </div>

      {/* ── Fullscreen overlay ──────────────────────────────────────────────── */}
      {fullscreenCamera && (
        <div
          className="fixed inset-0 z-50 bg-black/95 flex flex-col items-center justify-center"
          style={{ animation: "fs-fade-in 0.2s ease" }}
          onClick={(e) => {
            if (e.target === e.currentTarget) closeFullscreen();
          }}
          onMouseMove={resetControlsTimer}
        >
          {/* Controls bar */}
          <div
            className="absolute top-0 inset-x-0 flex items-center gap-3 p-3 z-10 transition-opacity duration-300"
            style={{
              background: "linear-gradient(to bottom, rgba(0,0,0,0.7), transparent)",
              opacity: controlsVisible ? 1 : 0,
            }}
          >
            <span className="text-sm font-bold uppercase tracking-wide text-white mr-auto">
              {fullscreenCamera.display_name}
            </span>
            <span style={rtcDotStyle()} />
            <button
              onClick={closeFullscreen}
              className="w-8 h-8 rounded-full flex items-center justify-center text-white text-lg transition-colors"
              style={{ background: "rgba(255,255,255,0.15)" }}
              onMouseOver={(e) => { (e.currentTarget as HTMLButtonElement).style.background = "rgba(255,255,255,0.3)"; }}
              onMouseOut={(e) => { (e.currentTarget as HTMLButtonElement).style.background = "rgba(255,255,255,0.15)"; }}
            >
              ×
            </button>
          </div>

          {/* Loading spinner */}
          {streamStatus === "connecting" && (
            <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-10">
              <div
                className="w-10 h-10 rounded-full animate-spin"
                style={{ border: "3px solid #333", borderTop: "3px solid #E10600" }}
              />
            </div>
          )}

          {/* H.265 WebCodecs canvas (primary — Chrome/Edge/Safari) */}
          {!useMjpegFallback && (
            <canvas
              ref={canvasRef}
              className="w-full h-full bg-black"
              style={{ objectFit: "contain" }}
            />
          )}

          {/* MJPEG fallback image (Firefox or WebCodecs failure) */}
          {useMjpegFallback && (
            /* eslint-disable-next-line @next/next/no-img-element */
            <img
              ref={mjpegImgRef}
              alt="Live MJPEG"
              className="w-full h-full object-contain bg-black"
            />
          )}

          {/* Fallback mode indicator */}
          {showFallback && (
            <div
              className="absolute bottom-1/4 left-1/2 -translate-x-1/2 text-[0.75rem] text-[#999] px-3.5 py-1.5 rounded z-10"
              style={{ background: "rgba(0,0,0,0.7)" }}
            >
              WebCodecs unavailable — using MJPEG (D1 quality)
            </div>
          )}
        </div>
      )}
    </DashboardLayout>
  );
}
