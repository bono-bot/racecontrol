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
      className={`flex flex-col border-l border-[#2A2A2A] bg-[#0A0A0A] transition-all duration-200 ease-in-out overflow-hidden ${
        isOpen ? "w-[60%] min-w-[480px] opacity-100" : "w-0 min-w-0 opacity-0"
      }`}
    >
      {isOpen && (
        <>
          {/* Panel Header */}
          <div className="flex items-center justify-between px-5 py-3 border-b border-[#2A2A2A] shrink-0">
            <div>
              <h2 className="text-xs font-semibold text-white uppercase tracking-[0.15em] font-sans">{title}</h2>
              {subtitle && <p className="text-[10px] text-zinc-600 mt-0.5">{subtitle}</p>}
            </div>
            <button
              onClick={onClose}
              className="text-zinc-600 hover:text-white transition-colors duration-200 p-1.5 rounded hover:bg-[#1E1E1E] cursor-pointer"
              aria-label="Close panel"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
