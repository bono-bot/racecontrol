"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import DashboardLayout from "@/components/DashboardLayout";

// ── Constants ──────────────────────────────────────────────────────────────────
const SENTRY_BASE = "http://192.168.31.27:8096";
const GO2RTC_WS = "ws://192.168.31.27:1984/api/ws";
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

type WebRtcStatus = "connecting" | "connected" | "failed" | "disconnected" | "closed";

interface WebRtcConnection {
  pc: RTCPeerConnection;
  ws: WebSocket;
}

// ── Pre-warm animation keyframes injected once ─────────────────────────────────
const PREWARM_STYLE = `
@keyframes prewarm-pulse {
  0%, 100% { outline-color: rgba(76, 175, 80, 0.3); }
  50%       { outline-color: rgba(76, 175, 80, 0.8); }
}
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

// ── WebRTC helper — core go2rtc signaling ─────────────────────────────────────
function connectWebRTC(
  streamName: string,
  onTrack: ((stream: MediaStream) => void) | null,
  onStatus: ((state: string) => void) | null,
): WebRtcConnection {
  const ws = new WebSocket(`${GO2RTC_WS}?src=${streamName}`);
  const pc = new RTCPeerConnection({
    iceServers: [{ urls: "stun:stun.l.google.com:19302" }],
  });

  pc.addTransceiver("video", { direction: "recvonly" });
  pc.addTransceiver("audio", { direction: "recvonly" });

  pc.ontrack = (e) => {
    if (onTrack) onTrack(e.streams[0]);
  };

  pc.onicecandidate = (e) => {
    if (e.candidate && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "webrtc/candidate", value: e.candidate.toJSON() }));
    }
  };

  pc.onconnectionstatechange = () => {
    if (onStatus) onStatus(pc.connectionState);
  };

  ws.onopen = () => {
    pc.createOffer()
      .then((offer) => pc.setLocalDescription(offer))
      .then(() => {
        ws.send(
          JSON.stringify({ type: "webrtc/offer", value: pc.localDescription?.sdp }),
        );
      })
      .catch((err) => {
        console.error("WebRTC offer failed:", err);
        if (onStatus) onStatus("failed");
      });
  };

  ws.onmessage = (ev) => {
    try {
      const msg = JSON.parse(ev.data as string) as { type: string; value: string | RTCIceCandidateInit };
      if (msg.type === "webrtc/answer") {
        pc.setRemoteDescription(
          new RTCSessionDescription({ type: "answer", sdp: msg.value as string }),
        ).catch((err) => console.error("setRemoteDescription failed:", err));
      } else if (msg.type === "webrtc/candidate") {
        pc.addIceCandidate(new RTCIceCandidate(msg.value as RTCIceCandidateInit)).catch((err) =>
          console.debug("addIceCandidate failed:", err),
        );
      }
    } catch {
      // ignore parse errors
    }
  };

  ws.onerror = () => {
    if (onStatus) onStatus("failed");
  };

  return { pc, ws };
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
  const [webrtcStatus, setWebrtcStatus] = useState<WebRtcStatus>("connecting");
  const [dragOverChannel, setDragOverChannel] = useState<number | null>(null);
  const [statusText, setStatusText] = useState<string>("Loading...");
  const [draggingChannel, setDraggingChannel] = useState<number | null>(null);
  const [showFallback, setShowFallback] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [preWarmingChannel, setPreWarmingChannel] = useState<number | null>(null);

  // ── Refs ─────────────────────────────────────────────────────────────────────
  const pcRef = useRef<RTCPeerConnection | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const preWarmPcRef = useRef<RTCPeerConnection | null>(null);
  const preWarmWsRef = useRef<WebSocket | null>(null);
  const preWarmChannelRef = useRef<number | null>(null);
  const preWarmTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const controlsHideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const refreshTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const imgRefs = useRef<Record<number, HTMLImageElement>>({});
  const dragSrcChannelRef = useRef<number | null>(null);
  const camerasRef = useRef<CameraInfo[]>([]);

  // Keep camerasRef in sync with cameras state (for interval callbacks)
  useEffect(() => {
    camerasRef.current = cameras;
  }, [cameras]);

  // ── Inject pre-warm animation styles ─────────────────────────────────────────
  useEffect(() => {
    const existing = document.getElementById("cameras-page-styles");
    if (!existing) {
      const style = document.createElement("style");
      style.id = "cameras-page-styles";
      style.textContent = PREWARM_STYLE;
      document.head.appendChild(style);
    }
    return () => {
      const el = document.getElementById("cameras-page-styles");
      if (el) el.remove();
    };
  }, []);

  // ── teardownRtc ───────────────────────────────────────────────────────────────
  const teardownRtc = useCallback(() => {
    if (pcRef.current) {
      pcRef.current.ontrack = null;
      pcRef.current.onicecandidate = null;
      pcRef.current.onconnectionstatechange = null;
      pcRef.current.close();
      pcRef.current = null;
    }
    if (wsRef.current) {
      wsRef.current.onmessage = null;
      wsRef.current.onclose = null;
      wsRef.current.close();
      wsRef.current = null;
    }
  }, []);

  // ── teardownPreWarm ───────────────────────────────────────────────────────────
  const teardownPreWarm = useCallback(() => {
    if (preWarmTimerRef.current) {
      clearTimeout(preWarmTimerRef.current);
      preWarmTimerRef.current = null;
    }
    if (preWarmPcRef.current) { preWarmPcRef.current.close(); preWarmPcRef.current = null; }
    if (preWarmWsRef.current) { preWarmWsRef.current.close(); preWarmWsRef.current = null; }
    preWarmChannelRef.current = null;
    setPreWarmingChannel(null);
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
      const streamName = `ch${camera.nvr_channel}`;
      teardownRtc();

      setFullscreenCamera(camera);
      setWebrtcStatus("connecting");
      setShowFallback(false);

      // Set poster snapshot as fallback
      if (videoRef.current && camera.nvr_channel) {
        videoRef.current.poster = `${SENTRY_BASE}/api/v1/cameras/nvr/${camera.nvr_channel}/snapshot?t=${Date.now()}`;
        videoRef.current.srcObject = null;
      }

      const onStatus = (state: string) => {
        setWebrtcStatus(state as WebRtcStatus);
        if (state === "failed" || state === "disconnected" || state === "closed") {
          setShowFallback(true);
          setTimeout(() => setShowFallback(false), 5000);
        }
      };

      // Check if pre-warm connection matches this camera
      if (
        preWarmChannelRef.current === camera.nvr_channel &&
        preWarmPcRef.current &&
        preWarmWsRef.current
      ) {
        pcRef.current = preWarmPcRef.current;
        wsRef.current = preWarmWsRef.current;
        preWarmPcRef.current = null;
        preWarmWsRef.current = null;
        preWarmChannelRef.current = null;
        setPreWarmingChannel(null);

        // Wire up existing pre-warm connection for fullscreen use
        pcRef.current.ontrack = (e) => {
          if (videoRef.current) {
            videoRef.current.srcObject = e.streams[0];
            setWebrtcStatus("connected");
          }
        };
        pcRef.current.onconnectionstatechange = () => {
          if (pcRef.current) onStatus(pcRef.current.connectionState);
        };

        // Check if tracks already arrived during pre-warm
        const receivers = pcRef.current.getReceivers();
        for (const receiver of receivers) {
          if (receiver.track && receiver.track.kind === "video") {
            const stream = new MediaStream([receiver.track]);
            if (videoRef.current) {
              videoRef.current.srcObject = stream;
              setWebrtcStatus("connected");
            }
            break;
          }
        }
      } else {
        teardownPreWarm();

        const conn = connectWebRTC(
          streamName,
          (stream) => {
            if (videoRef.current) {
              videoRef.current.srcObject = stream;
              setWebrtcStatus("connected");
            }
          },
          onStatus,
        );
        pcRef.current = conn.pc;
        wsRef.current = conn.ws;
      }

      resetControlsTimer();
    },
    [teardownRtc, teardownPreWarm, resetControlsTimer],
  );

  // ── Close fullscreen ──────────────────────────────────────────────────────────
  const closeFullscreen = useCallback(() => {
    teardownRtc();
    teardownPreWarm();
    setFullscreenCamera(null);
    setWebrtcStatus("connecting");
    setShowFallback(false);
    setControlsVisible(true);
    if (controlsHideTimerRef.current) clearTimeout(controlsHideTimerRef.current);
    if (videoRef.current) {
      videoRef.current.srcObject = null;
      videoRef.current.poster = "";
    }
  }, [teardownRtc, teardownPreWarm]);

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
    const onBeforeUnload = () => teardownRtc();
    const onVisibilityChange = () => {
      if (document.hidden) teardownRtc();
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
      if (preWarmTimerRef.current) clearTimeout(preWarmTimerRef.current);
      if (controlsHideTimerRef.current) clearTimeout(controlsHideTimerRef.current);
      teardownRtc();
      teardownPreWarm();
    };
  }, [closeFullscreen, teardownRtc, teardownPreWarm]);

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

  // ── Pre-warm handlers ─────────────────────────────────────────────────────────
  const handleTileMouseEnter = useCallback(
    (channel: number) => {
      if (!channel) return;
      preWarmTimerRef.current = setTimeout(() => {
        if (preWarmChannelRef.current === channel) return;
        teardownPreWarm();
        preWarmChannelRef.current = channel;
        setPreWarmingChannel(channel);
        const conn = connectWebRTC(`ch${channel}`, null, null);
        preWarmPcRef.current = conn.pc;
        preWarmWsRef.current = conn.ws;
      }, 500);
    },
    [teardownPreWarm],
  );

  const handleTileMouseLeave = useCallback(() => {
    if (preWarmTimerRef.current) {
      clearTimeout(preWarmTimerRef.current);
      preWarmTimerRef.current = null;
    }
    setPreWarmingChannel(null);
    // Keep pre-warm connection alive — don't tear it down on leave
  }, []);

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

  // ── WebRTC status dot styling ─────────────────────────────────────────────────
  const rtcDotStyle = (): React.CSSProperties => {
    if (webrtcStatus === "connecting") {
      return {
        width: 8, height: 8, borderRadius: "50%",
        background: "#ffc107",
        animation: "rtc-pulse 1s infinite",
        display: "inline-block",
        flexShrink: 0,
      };
    }
    if (webrtcStatus === "connected") {
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
                    const isPreWarming = preWarmingChannel === ch;

                    return (
                      <div
                        key={camera.name}
                        draggable
                        onDragStart={(e) => handleDragStart(e, ch)}
                        onDragOver={(e) => handleDragOver(e, ch)}
                        onDragLeave={handleDragLeave}
                        onDragEnd={handleDragEnd}
                        onDrop={(e) => handleDrop(e, ch)}
                        onMouseEnter={() => handleTileMouseEnter(ch)}
                        onMouseLeave={handleTileMouseLeave}
                        className={`relative aspect-video bg-rp-card rounded overflow-hidden cursor-grab ${
                          offline ? "opacity-40" : ""
                        } ${isDragging ? "opacity-60" : ""}`}
                        style={{
                          display: isCollapsed ? "none" : undefined,
                          outline: isDragOver
                            ? "2px dashed #E10600"
                            : isPreWarming
                            ? "2px solid rgba(76, 175, 80, 0.6)"
                            : undefined,
                          outlineOffset: "-2px",
                          animation: isPreWarming ? "prewarm-pulse 1.5s ease-in-out infinite" : undefined,
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
          {webrtcStatus === "connecting" && (
            <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-10">
              <div
                className="w-10 h-10 rounded-full animate-spin"
                style={{ border: "3px solid #333", borderTop: "3px solid #E10600" }}
              />
            </div>
          )}

          {/* Video element */}
          <video
            ref={videoRef}
            autoPlay
            playsInline
            muted
            className="w-full h-full object-contain bg-black"
          />

          {/* Fallback message */}
          {showFallback && (
            <div
              className="absolute bottom-1/4 left-1/2 -translate-x-1/2 text-[0.75rem] text-[#999] px-3.5 py-1.5 rounded z-10"
              style={{ background: "rgba(0,0,0,0.7)" }}
            >
              Live unavailable — showing snapshot
            </div>
          )}
        </div>
      )}
    </DashboardLayout>
  );
}
