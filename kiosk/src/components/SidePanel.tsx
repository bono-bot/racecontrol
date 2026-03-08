"use client";

import { useEffect, useCallback, type ReactNode } from "react";

interface SidePanelProps {
  isOpen: boolean;
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
}

export function SidePanel({ isOpen, title, subtitle, onClose, children }: SidePanelProps) {
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose]
  );

  useEffect(() => {
    if (isOpen) {
      document.addEventListener("keydown", handleKeyDown);
      return () => document.removeEventListener("keydown", handleKeyDown);
    }
  }, [isOpen, handleKeyDown]);

  return (
    <div
      className={`flex flex-col border-l border-rp-border bg-rp-card transition-all duration-300 overflow-hidden ${
        isOpen ? "w-[60%] min-w-[480px] opacity-100" : "w-0 min-w-0 opacity-0"
      }`}
    >
      {isOpen && (
        <>
          {/* Panel Header */}
          <div className="flex items-center justify-between px-5 py-3 border-b border-rp-border shrink-0">
            <div>
              <h2 className="text-sm font-semibold text-white">{title}</h2>
              {subtitle && <p className="text-xs text-rp-grey">{subtitle}</p>}
            </div>
            <button
              onClick={onClose}
              className="text-rp-grey hover:text-white transition-colors p-1"
              aria-label="Close panel"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          {/* Panel Content */}
          <div className="flex-1 overflow-y-auto">
            {children}
          </div>
        </>
      )}
    </div>
  );
}
