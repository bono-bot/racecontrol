"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import Link from "next/link";

const SENTRY_BASE = "http://192.168.31.27:8096";

interface CameraInfo {
  name: string;
  role: string;
  stream_url: string;
  status: string;
}

interface NvrFileInfo {
  channel: number;
  start_time: string;
  end_time: string;
  file_path: string;
  file_size: number;
  file_type: string;
}

interface AttendanceEntry {
  id: number;
  person_id: string;
  person_name: string;
  camera_id: string;
  confidence: number;
  logged_at: string;
  day: string;
}

function formatFileSize(bytes: number): string {
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + " GB";
  if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + " MB";
  return (bytes / 1024).toFixed(1) + " KB";
}

function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString("en-IN", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
  } catch {
    return iso;
  }
}

function todayIST(): string {
  const now = new Date();
  const ist = new Date(now.getTime() + 5.5 * 60 * 60 * 1000);
  return ist.toISOString().slice(0, 10);
}

export default function PlaybackPage() {
  const [cameras, setCameras] = useState<CameraInfo[]>([]);
  const [selectedCamera, setSelectedCamera] = useState("");
  const [date, setDate] = useState("");
  const [startTime, setStartTime] = useState("00:00");
  const [endTime, setEndTime] = useState("23:59");
  const [files, setFiles] = useState<NvrFileInfo[]>([]);
  const [selectedFile, setSelectedFile] = useState<NvrFileInfo | null>(null);
  const [events, setEvents] = useState<AttendanceEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [hydrated, setHydrated] = useState(false);

  // Hydration guard for date default
  useEffect(() => {
    setDate(todayIST());
    setHydrated(true);
  }, []);

  // Fetch camera list on mount
  const fetchCameras = useCallback(async () => {
    try {
      const res = await fetch(`${SENTRY_BASE}/api/v1/cameras`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: CameraInfo[] = await res.json();
      setCameras(data);
      if (data.length > 0 && !selectedCamera) {
        setSelectedCamera(data[0].name);
      }
    } catch (err) {
      console.error("Failed to fetch cameras:", err);
    }
  }, [selectedCamera]);

  useEffect(() => {
    fetchCameras();
  }, [fetchCameras]);

  // Fetch events when date changes
  useEffect(() => {
    if (!date) return;
    const fetchEvents = async () => {
      try {
        const res = await fetch(
          `${SENTRY_BASE}/api/v1/playback/events?day=${date}`
        );
        if (!res.ok) {
          setEvents([]);
          return;
        }
        const data: AttendanceEntry[] = await res.json();
        setEvents(data);
      } catch {
        setEvents([]);
      }
    };
    fetchEvents();
  }, [date]);

  const handleSearch = async () => {
    if (!selectedCamera || !date) return;
    setLoading(true);
    setError(null);
    setFiles([]);
    setSelectedFile(null);
    try {
      const startISO = `${date}T${startTime}:00`;
      const endISO = `${date}T${endTime}:00`;
      const params = new URLSearchParams({
        camera: selectedCamera,
        start: startISO,
        end: endISO,
      });
      const res = await fetch(
        `${SENTRY_BASE}/api/v1/playback/search?${params}`
      );
      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || `HTTP ${res.status}`);
      }
      const data: NvrFileInfo[] = await res.json();
      setFiles(data);
      if (data.length === 0) {
        setError("No recordings found for the selected time range.");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Search failed");
    } finally {
      setLoading(false);
    }
  };

  const selectFile = (file: NvrFileInfo) => {
    setSelectedFile(file);
  };

  const streamUrl = selectedFile
    ? `${SENTRY_BASE}/api/v1/playback/stream?file_path=${encodeURIComponent(selectedFile.file_path)}`
    : null;

  // Event timeline: compute position as percentage within the search time range
  const timeToMinutes = (timeStr: string): number => {
    const parts = timeStr.split(":");
    return parseInt(parts[0], 10) * 60 + parseInt(parts[1], 10);
  };

  const rangeStartMin = timeToMinutes(startTime);
  const rangeEndMin = timeToMinutes(endTime);
  const rangeDuration = rangeEndMin - rangeStartMin;

  const getEventPosition = (loggedAt: string): number => {
    try {
      const d = new Date(loggedAt);
      const eventMin = d.getHours() * 60 + d.getMinutes();
      if (rangeDuration <= 0) return 0;
      const pos = ((eventMin - rangeStartMin) / rangeDuration) * 100;
      return Math.max(0, Math.min(100, pos));
    } catch {
      return 0;
    }
  };

  const handleEventClick = (event: AttendanceEntry) => {
    // Find the recording file that contains this event's timestamp
    const eventTime = new Date(event.logged_at);
    const match = files.find((f) => {
      const fStart = new Date(f.start_time);
      const fEnd = new Date(f.end_time);
      return eventTime >= fStart && eventTime <= fEnd;
    });
    if (match) {
      setSelectedFile(match);
      // Seek video to the event offset
      if (videoRef.current) {
        const fStart = new Date(match.start_time);
        const offsetSec = (eventTime.getTime() - fStart.getTime()) / 1000;
        videoRef.current.currentTime = Math.max(0, offsetSec);
      }
    }
  };

  const filteredEvents = events.filter((e) => {
    const pos = getEventPosition(e.logged_at);
    return pos >= 0 && pos <= 100;
  });

  if (!hydrated) {
    return (
      <DashboardLayout>
        <div className="text-center text-rp-grey py-16">
          <p className="animate-pulse">Loading...</p>
        </div>
      </DashboardLayout>
    );
  }

  return (
    <DashboardLayout>
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold">NVR Playback</h1>
          <p className="text-sm text-rp-grey">Review recorded footage</p>
        </div>
        <Link
          href="/cameras"
          className="px-3 py-1.5 rounded-lg text-xs font-medium bg-rp-card text-rp-grey border border-rp-border hover:text-white transition-colors"
        >
          &larr; Live Cameras
        </Link>
      </div>

      {/* Search Form */}
      <div className="bg-rp-card border border-rp-border rounded-lg p-4 mb-6">
        <div className="flex flex-wrap items-end gap-4">
          <div className="flex-1 min-w-[140px]">
            <label className="block text-xs text-rp-grey mb-1">Camera</label>
            <select
              value={selectedCamera}
              onChange={(e) => setSelectedCamera(e.target.value)}
              className="w-full bg-rp-black border border-rp-border rounded-lg px-3 py-2 text-sm text-white"
            >
              {cameras.map((cam) => (
                <option key={cam.name} value={cam.name}>
                  {cam.name}
                </option>
              ))}
            </select>
          </div>
          <div className="min-w-[140px]">
            <label className="block text-xs text-rp-grey mb-1">Date</label>
            <input
              type="date"
              value={date}
              onChange={(e) => setDate(e.target.value)}
              className="w-full bg-rp-black border border-rp-border rounded-lg px-3 py-2 text-sm text-white"
            />
          </div>
          <div className="min-w-[100px]">
            <label className="block text-xs text-rp-grey mb-1">From</label>
            <input
              type="time"
              value={startTime}
              onChange={(e) => setStartTime(e.target.value)}
              className="w-full bg-rp-black border border-rp-border rounded-lg px-3 py-2 text-sm text-white"
            />
          </div>
          <div className="min-w-[100px]">
            <label className="block text-xs text-rp-grey mb-1">To</label>
            <input
              type="time"
              value={endTime}
              onChange={(e) => setEndTime(e.target.value)}
              className="w-full bg-rp-black border border-rp-border rounded-lg px-3 py-2 text-sm text-white"
            />
          </div>
          <button
            onClick={handleSearch}
            disabled={loading || !selectedCamera}
            className="bg-[#E10600] text-white rounded-lg px-4 py-2 text-sm font-medium hover:bg-[#C10500] transition-colors disabled:opacity-50"
          >
            {loading ? "Searching..." : "Search"}
          </button>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="bg-rp-card border border-red-500/30 rounded-lg p-3 mb-4">
          <p className="text-red-400 text-sm">{error}</p>
        </div>
      )}

      {/* Results: File list + Video player */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4 mb-6">
        {/* File List */}
        <div className="lg:col-span-1">
          <h2 className="text-sm font-bold text-rp-grey mb-2">
            Recordings ({files.length})
          </h2>
          <div className="space-y-2 max-h-[500px] overflow-y-auto">
            {files.length === 0 && !loading && !error && (
              <p className="text-xs text-rp-grey py-4 text-center">
                Search for recordings above
              </p>
            )}
            {files.map((file, idx) => (
              <button
                key={idx}
                onClick={() => selectFile(file)}
                className={`w-full text-left bg-rp-card border rounded-lg p-3 transition-colors ${
                  selectedFile?.file_path === file.file_path
                    ? "border-[#E10600]"
                    : "border-rp-border hover:border-rp-grey"
                }`}
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-xs font-medium text-white">
                    {formatTime(file.start_time)} &mdash;{" "}
                    {formatTime(file.end_time)}
                  </span>
                  <span className="text-[10px] text-rp-grey">
                    {formatFileSize(file.file_size)}
                  </span>
                </div>
                <p className="text-[10px] text-rp-grey truncate">
                  Ch{file.channel} &middot; {file.file_type}
                </p>
              </button>
            ))}
          </div>
        </div>

        {/* Video Player */}
        <div className="lg:col-span-2">
          <h2 className="text-sm font-bold text-rp-grey mb-2">Player</h2>
          {streamUrl ? (
            <video
              ref={videoRef}
              key={streamUrl}
              src={streamUrl}
              controls
              className="w-full aspect-video bg-black rounded-lg"
            >
              Your browser does not support video playback.
            </video>
          ) : (
            <div className="w-full aspect-video bg-rp-card border border-rp-border rounded-lg flex items-center justify-center">
              <p className="text-rp-grey text-sm">Select a recording to play</p>
            </div>
          )}
        </div>
      </div>

      {/* Event Timeline */}
      <div className="bg-rp-card border border-rp-border rounded-lg p-4">
        <h2 className="text-sm font-bold text-rp-grey mb-3">
          Attendance Events
        </h2>
        {filteredEvents.length === 0 ? (
          <p className="text-xs text-rp-grey">
            No attendance events for this date
          </p>
        ) : (
          <div className="relative">
            {/* Time labels */}
            <div className="flex justify-between text-[10px] text-rp-grey mb-1">
              <span>{startTime}</span>
              <span>{endTime}</span>
            </div>
            {/* Timeline bar */}
            <div className="relative h-8 bg-rp-black border border-rp-border rounded">
              {filteredEvents.map((event) => {
                const pos = getEventPosition(event.logged_at);
                return (
                  <button
                    key={event.id}
                    onClick={() => handleEventClick(event)}
                    title={`${event.person_name} (${event.camera_id}) - ${(event.confidence * 100).toFixed(0)}% - ${formatTime(event.logged_at)}`}
                    className="absolute top-1/2 -translate-y-1/2 w-3 h-3 rounded-full bg-[#E10600] hover:bg-[#FF2020] border border-white/30 transition-colors cursor-pointer group"
                    style={{ left: `${pos}%` }}
                  >
                    {/* Tooltip */}
                    <span className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 hidden group-hover:block whitespace-nowrap bg-rp-black border border-rp-border rounded px-2 py-1 text-[10px] text-white z-10">
                      {event.person_name} &middot; {event.camera_id} &middot;{" "}
                      {(event.confidence * 100).toFixed(0)}% &middot;{" "}
                      {formatTime(event.logged_at)}
                    </span>
                  </button>
                );
              })}
            </div>
            <p className="text-[10px] text-rp-grey mt-1">
              {filteredEvents.length} event
              {filteredEvents.length !== 1 ? "s" : ""} &mdash; click a marker to
              jump to recording
            </p>
          </div>
        )}
      </div>
    </DashboardLayout>
  );
}
