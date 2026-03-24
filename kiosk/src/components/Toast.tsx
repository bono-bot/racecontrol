"use client";

import { createContext, useContext, useState, useCallback, type ReactNode } from "react";

// ─── Types ──────────────────────────────────────────────────────────────────

type ToastType = "error" | "success" | "info" | "warning";

interface ToastItem {
  id: number;
  message: string;
  type: ToastType;
}

interface ToastContextValue {
  toast: (message: string, type?: ToastType) => void;
  toastError: (message: string) => void;
  toastSuccess: (message: string) => void;
}

// ─── Context ────────────────────────────────────────────────────────────────

const ToastContext = createContext<ToastContextValue | null>(null);

let nextId = 0;
const TOAST_DURATION_MS = 5_000;

// ─── Provider ───────────────────────────────────────────────────────────────

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const removeToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toast = useCallback(
    (message: string, type: ToastType = "info") => {
      const id = nextId++;
      setToasts((prev) => [...prev, { id, message, type }]);
      setTimeout(() => removeToast(id), TOAST_DURATION_MS);
    },
    [removeToast]
  );

  const toastError = useCallback(
    (message: string) => toast(message, "error"),
    [toast]
  );

  const toastSuccess = useCallback(
    (message: string) => toast(message, "success"),
    [toast]
  );

  return (
    <ToastContext.Provider value={{ toast, toastError, toastSuccess }}>
      {children}

      {/* Toast container — fixed bottom-right */}
      {toasts.length > 0 && (
        <div className="fixed bottom-4 right-4 z-[9999] flex flex-col gap-2 max-w-sm">
          {toasts.map((t) => (
            <div
              key={t.id}
              className={`flex items-start gap-3 px-4 py-3 rounded-lg border shadow-lg text-sm animate-[slideUp_0.2s_ease-out] ${toastStyles[t.type]}`}
              onClick={() => removeToast(t.id)}
            >
              <span className="mt-0.5">{toastIcons[t.type]}</span>
              <span className="flex-1">{t.message}</span>
              <button className="text-white/50 hover:text-white transition-colors ml-2">
                ✕
              </button>
            </div>
          ))}
        </div>
      )}
    </ToastContext.Provider>
  );
}

// ─── Hook ───────────────────────────────────────────────────────────────────

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    // Fallback for pages without the provider — console.warn + noop
    return {
      toast: (msg, type) => console.warn(`[Toast/${type || "info"}]`, msg),
      toastError: (msg) => console.error("[Toast/error]", msg),
      toastSuccess: (msg) => console.log("[Toast/success]", msg),
    };
  }
  return ctx;
}

// ─── Styles ─────────────────────────────────────────────────────────────────

const toastStyles: Record<ToastType, string> = {
  error: "bg-red-900/90 border-red-700 text-red-100",
  success: "bg-green-900/90 border-green-700 text-green-100",
  info: "bg-zinc-800/90 border-zinc-600 text-zinc-100",
  warning: "bg-amber-900/90 border-amber-700 text-amber-100",
};

const toastIcons: Record<ToastType, string> = {
  error: "✕",
  success: "✓",
  info: "ℹ",
  warning: "⚠",
};
