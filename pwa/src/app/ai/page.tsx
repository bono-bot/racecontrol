"use client";

import { useState, useRef, useEffect } from "react";
import { api, isLoggedIn } from "@/lib/api";
import { useRouter } from "next/navigation";
import BottomNav from "@/components/BottomNav";

interface Message {
  role: "user" | "assistant";
  content: string;
}

const SUGGESTIONS = [
  "What was the fastest lap of the day?",
  "Show me my personal bests",
  "What are the pricing options?",
  "Give me tips to improve my lap times",
];

export default function AiChatPage() {
  const router = useRouter();
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!isLoggedIn()) router.replace("/login");
  }, [router]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  async function sendMessage(text: string) {
    if (!text.trim() || loading) return;

    const userMsg: Message = { role: "user", content: text.trim() };
    setMessages((prev) => [...prev, userMsg]);
    setInput("");
    setLoading(true);

    try {
      const history = messages.map((m) => ({ role: m.role, content: m.content }));
      const data = await api.aiChat(text.trim(), history);
      if (data.error) {
        setMessages((prev) => [...prev, { role: "assistant", content: data.error! }]);
      } else {
        setMessages((prev) => [
          ...prev,
          { role: "assistant", content: data.reply || "No response." },
        ]);
      }
    } catch {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: "AI service is unavailable right now. Please try again later." },
      ]);
    }
    setLoading(false);
    inputRef.current?.focus();
  }

  async function handleSend(e: React.FormEvent) {
    e.preventDefault();
    sendMessage(input);
  }

  return (
    <div className="flex flex-col h-screen pb-16">
      {/* Header */}
      <div className="px-4 pt-6 pb-3">
        <h1 className="text-xl font-bold">RacingPoint Bot AI</h1>
        <p className="text-xs text-rp-grey">Your racing assistant — powered by Bono</p>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-auto px-4 space-y-3">
        {messages.length === 0 && (
          <div className="text-center text-rp-grey py-10">
            <div className="text-4xl mb-3">&#127937;</div>
            <p className="text-sm font-medium mb-1">Hey, I&apos;m Bono!</p>
            <p className="text-xs leading-relaxed mb-6">
              Ask about your lap times, stats,<br />
              pricing, or sim racing tips.
            </p>
            <div className="flex flex-wrap justify-center gap-2 px-2">
              {SUGGESTIONS.map((s) => (
                <button
                  key={s}
                  onClick={() => sendMessage(s)}
                  className="text-xs bg-rp-card border border-rp-border rounded-full px-3 py-1.5 text-neutral-300 hover:border-rp-red hover:text-white transition-colors text-left"
                >
                  {s}
                </button>
              ))}
            </div>
          </div>
        )}
        {messages.map((m, i) => (
          <div key={i} className={`flex ${m.role === "user" ? "justify-end" : "justify-start"}`}>
            <div
              className={`max-w-[85%] rounded-2xl px-4 py-2.5 text-sm ${
                m.role === "user"
                  ? "bg-rp-red/20 text-red-100 border border-rp-red/30"
                  : "bg-rp-card border border-rp-border text-neutral-200"
              }`}
            >
              <p className="whitespace-pre-wrap">{m.content}</p>
            </div>
          </div>
        ))}
        {loading && (
          <div className="flex justify-start">
            <div className="bg-rp-card border border-rp-border rounded-2xl px-4 py-2.5 text-sm text-rp-grey">
              <span className="animate-pulse">Thinking...</span>
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {/* Input */}
      <form onSubmit={handleSend} className="p-3 flex gap-2 bg-rp-dark border-t border-rp-border">
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Ask Bono..."
          className="flex-1 bg-rp-card border border-rp-border rounded-full px-4 py-2.5 text-sm focus:outline-none focus:border-rp-red text-white placeholder-rp-grey"
          disabled={loading}
        />
        <button
          type="submit"
          disabled={loading || !input.trim()}
          className="w-10 h-10 flex items-center justify-center bg-rp-red hover:bg-rp-red-light disabled:bg-rp-border disabled:text-rp-grey rounded-full transition-colors"
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M5 12h14m-7-7l7 7-7 7" />
          </svg>
        </button>
      </form>

      <BottomNav />
    </div>
  );
}
