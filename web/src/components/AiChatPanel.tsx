"use client";

import { useState, useRef, useEffect } from "react";
import { api } from "@/lib/api";

interface Message {
  role: "user" | "assistant";
  content: string;
}

export default function AiChatPanel() {
  const [open, setOpen] = useState(false);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [model, setModel] = useState<string | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  async function handleSend(e: React.FormEvent) {
    e.preventDefault();
    if (!input.trim() || loading) return;

    const userMsg: Message = { role: "user", content: input.trim() };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");
    setLoading(true);

    try {
      const history = messages.map((m) => ({ role: m.role, content: m.content }));
      const data = await api.aiChat(input.trim(), history);
      if (data.error) {
        setMessages((prev) => [...prev, { role: "assistant", content: data.error! }]);
      } else {
        setMessages((prev) => [...prev, { role: "assistant", content: data.reply || "No response." }]);
        if (data.model) setModel(data.model);
      }
    } catch {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: "AI service unavailable. Check that Ollama is running." },
      ]);
    }
    setLoading(false);
  }

  return (
    <>
      {/* Floating button */}
      <button
        onClick={() => setOpen(!open)}
        className={`fixed bottom-6 right-6 z-50 w-14 h-14 rounded-full shadow-lg flex items-center justify-center transition-all ${
          open
            ? "bg-rp-border text-neutral-400 hover:bg-neutral-600"
            : "bg-violet-600 text-white hover:bg-violet-500"
        }`}
      >
        {open ? (
          <span className="text-xl">&times;</span>
        ) : (
          <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M8 10h.01M12 10h.01M16 10h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
          </svg>
        )}
      </button>

      {/* Chat panel */}
      <div
        className={`fixed top-0 right-0 z-40 h-full w-96 bg-rp-black border-l border-rp-border flex flex-col transition-transform duration-300 ${
          open ? "translate-x-0" : "translate-x-full"
        }`}
      >
        {/* Header */}
        <div className="px-4 py-3 border-b border-rp-border flex items-center justify-between">
          <div>
            <h2 className="text-sm font-semibold text-violet-300">James AI</h2>
            <p className="text-xs text-rp-grey">
              {model ? model : "RaceControl Assistant"}
            </p>
          </div>
          <button
            onClick={() => {
              setMessages([]);
              setModel(null);
            }}
            className="text-xs text-rp-grey hover:text-white transition-colors"
          >
            Clear
          </button>
        </div>

        {/* Messages */}
        <div className="flex-1 overflow-auto p-4 space-y-3">
          {messages.length === 0 && (
            <div className="text-center text-rp-grey py-12">
              <p className="text-sm mb-1">Hey, I&apos;m James!</p>
              <p className="text-xs">Ask about pods, sessions, revenue, crashes...</p>
            </div>
          )}
          {messages.map((m, i) => (
            <div key={i} className={`flex ${m.role === "user" ? "justify-end" : "justify-start"}`}>
              <div
                className={`max-w-[85%] rounded-lg px-3 py-2 text-sm ${
                  m.role === "user"
                    ? "bg-violet-600/20 text-violet-100 border border-violet-500/30"
                    : "bg-rp-card border border-rp-border text-neutral-200"
                }`}
              >
                <p className="whitespace-pre-wrap">{m.content}</p>
              </div>
            </div>
          ))}
          {loading && (
            <div className="flex justify-start">
              <div className="bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-rp-grey">
                <span className="animate-pulse">Thinking...</span>
              </div>
            </div>
          )}
          <div ref={bottomRef} />
        </div>

        {/* Input */}
        <form onSubmit={handleSend} className="p-3 border-t border-rp-border flex gap-2">
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Ask James..."
            className="flex-1 bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-violet-500 text-white placeholder-rp-grey"
            disabled={loading}
          />
          <button
            type="submit"
            disabled={loading || !input.trim()}
            className="px-4 py-2 bg-violet-600 hover:bg-violet-500 disabled:bg-rp-border disabled:text-rp-grey rounded-lg text-sm font-medium transition-colors"
          >
            Send
          </button>
        </form>
      </div>
    </>
  );
}
