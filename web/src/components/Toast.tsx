"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";

// --- Types ---

type ToastType = "success" | "error" | "warning" | "info";

interface ToastItem {
  id: string;
  message: string;
  type: ToastType;
  duration: number;
}

interface ToastOpts {
  message: string;
  type?: ToastType;
  duration?: number;
}

interface ToastContextValue {
  toast: (opts: ToastOpts) => void;
  dismiss: (id: string) => void;
}

// --- Context ---

const ToastContext = createContext<ToastContextValue | null>(null);

// --- Icons (inline SVG) ---

function CheckIcon() {
  return (
    <svg
      className="w-4 h-4 text-rp-green shrink-0 mt-0.5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth={2}
    >
      <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
    </svg>
  );
}

function XIcon() {
  return (
    <svg
      className="w-4 h-4 text-rp-red shrink-0 mt-0.5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth={2}
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M6 18L18 6M6 6l12 12"
      />
    </svg>
  );
}

function WarningIcon() {
  return (
    <svg
      className="w-4 h-4 text-rp-yellow shrink-0 mt-0.5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth={2}
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
      />
    </svg>
  );
}

function InfoIcon() {
  return (
    <svg
      className="w-4 h-4 text-blue-400 shrink-0 mt-0.5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth={2}
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
      />
    </svg>
  );
}

const ICON_MAP: Record<ToastType, () => ReactNode> = {
  success: CheckIcon,
  error: XIcon,
  warning: WarningIcon,
  info: InfoIcon,
};

const BORDER_MAP: Record<ToastType, string> = {
  success: "border-rp-green/50",
  error: "border-rp-red/50",
  warning: "border-rp-yellow/50",
  info: "border-rp-border",
};

const DEFAULT_DURATION: Record<ToastType, number> = {
  success: 4000,
  error: 6000,
  warning: 4000,
  info: 4000,
};

// --- Single Toast ---

function ToastCard({
  item,
  onDismiss,
}: {
  item: ToastItem;
  onDismiss: (id: string) => void;
}) {
  useEffect(() => {
    const timer = setTimeout(() => onDismiss(item.id), item.duration);
    return () => clearTimeout(timer);
  }, [item.id, item.duration, onDismiss]);

  const Icon = ICON_MAP[item.type];
  const borderClass = BORDER_MAP[item.type];

  return (
    <div
      className={`pointer-events-auto flex items-start gap-3 px-4 py-3 rounded-lg border shadow-lg text-sm bg-rp-card ${borderClass} text-white animate-in slide-in-from-right-5 fade-in duration-200`}
      role="alert"
    >
      <Icon />
      <span className="flex-1">{item.message}</span>
      <button
        onClick={() => onDismiss(item.id)}
        className="text-rp-grey hover:text-white transition-colors shrink-0 mt-0.5"
        aria-label="Dismiss"
      >
        <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
}

// --- Provider ---

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const counterRef = useRef(0);

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toast = useCallback((opts: ToastOpts) => {
    const type = opts.type ?? "info";
    const duration = opts.duration ?? DEFAULT_DURATION[type];
    const id = `toast-${++counterRef.current}-${Date.now()}`;

    setToasts((prev) => {
      const next = [...prev, { id, message: opts.message, type, duration }];
      // Max 5 visible at once
      return next.length > 5 ? next.slice(-5) : next;
    });
  }, []);

  return (
    <ToastContext.Provider value={{ toast, dismiss }}>
      {children}
      {/* Toast stack */}
      <div className="fixed top-4 right-4 z-50 flex flex-col gap-2 pointer-events-none w-80">
        {toasts.map((item) => (
          <ToastCard key={item.id} item={item} onDismiss={dismiss} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

// --- Hook ---

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error("useToast must be used within a ToastProvider");
  }
  return ctx;
}
