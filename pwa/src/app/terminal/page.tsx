"use client";

import { useEffect, useRef, useState } from "react";
import { api } from "@/lib/api";
import type { TerminalCommand } from "@/lib/api";

export default function TerminalPage() {
  const [session, setSession] = useState<string | null>(null);
  const [pin, setPin] = useState("");
  const [pinError, setPinError] = useState("");
  const [authenticating, setAuthenticating] = useState(false);

  // Check for existing session on mount
  useEffect(() => {
    const saved = sessionStorage.getItem("terminal_session");
    if (saved) setSession(saved);
  }, []);

  async function handlePinSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!pin.trim() || authenticating) return;
    setAuthenticating(true);
    setPinError("");

    const res = await api.terminalAuth(pin.trim());
    if (res.session) {
      sessionStorage.setItem("terminal_session", res.session);
      setSession(res.session);
    } else {
      setPinError(res.error || "Invalid PIN");
    }
    setAuthenticating(false);
  }

  if (!session) {
    return (
      <div className="flex items-center justify-center h-screen bg-black">
        <form
          onSubmit={handlePinSubmit}
          className="flex flex-col items-center gap-6 p-8 bg-neutral-900 rounded-2xl border border-neutral-800 w-80"
        >
          <div className="w-12 h-12 rounded-full bg-rp-red/20 flex items-center justify-center">
            <svg
              className="w-6 h-6 text-rp-red"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"
              />
            </svg>
          </div>
          <div className="text-center">
            <h1 className="text-white text-lg font-bold font-mono">
              James Terminal
            </h1>
            <p className="text-neutral-500 text-sm mt-1">
              Enter PIN to access
            </p>
          </div>
          <input
            type="password"
            inputMode="numeric"
            value={pin}
            onChange={(e) => setPin(e.target.value)}
            placeholder="PIN"
            autoFocus
            maxLength={16}
            className="w-full text-center text-2xl tracking-[0.5em] bg-neutral-800 border border-neutral-700 rounded-lg px-4 py-3 text-white font-mono outline-none focus:border-rp-red transition-colors"
          />
          {pinError && (
            <p className="text-red-500 text-sm -mt-3">{pinError}</p>
          )}
          <button
            type="submit"
            disabled={authenticating || !pin.trim()}
            className="w-full bg-rp-red text-white py-2.5 rounded-lg font-medium disabled:opacity-30 transition-opacity"
          >
            {authenticating ? "Verifying..." : "Unlock"}
          </button>
        </form>
      </div>
    );
  }

  return <Terminal session={session} onSessionExpired={() => {
    sessionStorage.removeItem("terminal_session");
    setSession(null);
    setPin("");
  }} />;
}

function Terminal({ session, onSessionExpired }: { session: string; onSessionExpired: () => void }) {
  const [commands, setCommands] = useState<TerminalCommand[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Poll for command updates
  useEffect(() => {
    const poll = async () => {
      const res = await api.terminalList(30, session);
      if (res.error?.includes("Unauthorized")) {
        onSessionExpired();
        return;
      }
      if (res.commands) {
        setCommands(res.commands.reverse());
      }
    };
    poll();
    const interval = setInterval(poll, 2000);
    return () => clearInterval(interval);
  }, [session, onSessionExpired]);

  // Auto-scroll to bottom on new commands
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [commands]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const cmd = input.trim();
    if (!cmd || sending) return;

    setSending(true);
    setInput("");
    const res = await api.terminalSubmit(cmd, 30000, session);
    if (res.error?.includes("Unauthorized")) {
      onSessionExpired();
      return;
    }
    setSending(false);

    // Quick refresh
    const listRes = await api.terminalList(30, session);
    if (listRes.commands) setCommands(listRes.commands.reverse());
    inputRef.current?.focus();
  }

  return (
    <div className="flex flex-col h-screen bg-black">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 bg-neutral-900 border-b border-neutral-800">
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded-full bg-rp-red" />
          <span className="text-sm font-mono text-neutral-300">
            james@racingpoint
          </span>
        </div>
        <span className="text-xs text-neutral-600 font-mono">
          Cloud Terminal
        </span>
      </div>

      {/* Output area */}
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto px-4 py-3 font-mono text-sm space-y-4"
      >
        {commands.length === 0 && (
          <p className="text-neutral-600">
            Type a command below to execute on James (192.168.31.35)
          </p>
        )}
        {commands.map((cmd) => (
          <CommandBlock key={cmd.id} cmd={cmd} />
        ))}
      </div>

      {/* Input */}
      <form
        onSubmit={handleSubmit}
        className="flex items-center gap-2 px-4 py-3 bg-neutral-900 border-t border-neutral-800"
      >
        <span className="text-rp-red font-mono text-sm font-bold">$</span>
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Type a command..."
          autoFocus
          disabled={sending}
          className="flex-1 bg-transparent text-white font-mono text-sm outline-none placeholder-neutral-600 disabled:opacity-50"
        />
        <button
          type="submit"
          disabled={sending || !input.trim()}
          className="text-xs bg-rp-red text-white px-3 py-1.5 rounded font-medium disabled:opacity-30"
        >
          Run
        </button>
      </form>
    </div>
  );
}

function CommandBlock({ cmd }: { cmd: TerminalCommand }) {
  const isPending = cmd.status === "pending" || cmd.status === "running";
  const isFailed = cmd.status === "failed" || cmd.status === "timeout";

  return (
    <div>
      {/* Command line */}
      <div className="flex items-center gap-2">
        <span className="text-rp-red font-bold">$</span>
        <span className="text-neutral-200">{cmd.cmd}</span>
        {isPending && (
          <span className="inline-block w-3 h-3 border border-neutral-500 border-t-rp-red rounded-full animate-spin ml-2" />
        )}
        {cmd.exit_code !== null && cmd.exit_code !== 0 && (
          <span className="text-red-500 text-xs ml-2">
            exit {cmd.exit_code}
          </span>
        )}
      </div>

      {/* Output */}
      {cmd.stdout && (
        <pre className="text-neutral-400 text-xs mt-1 whitespace-pre-wrap break-all leading-relaxed">
          {cmd.stdout}
        </pre>
      )}
      {cmd.stderr && !isPending && (
        <pre
          className={`text-xs mt-1 whitespace-pre-wrap break-all leading-relaxed ${
            isFailed ? "text-red-400" : "text-yellow-600"
          }`}
        >
          {cmd.stderr}
        </pre>
      )}

      {/* Timestamp */}
      <div className="text-neutral-700 text-[10px] mt-1">
        {formatTime(cmd.created_at)}
        {cmd.completed_at && ` — ${formatDuration(cmd.created_at, cmd.completed_at)}`}
      </div>
    </div>
  );
}

function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString("en-IN", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  } catch {
    return iso;
  }
}

function formatDuration(start: string, end: string): string {
  try {
    const ms = new Date(end).getTime() - new Date(start).getTime();
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  } catch {
    return "";
  }
}
