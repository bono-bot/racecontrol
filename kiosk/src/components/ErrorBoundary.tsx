"use client";

import { Component, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  componentDidCatch(error: Error) {
    console.error("[ErrorBoundary] Caught:", error);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="h-screen w-screen flex flex-col items-center justify-center gap-6 bg-rp-black">
          <div className="w-20 h-20 rounded-full bg-red-900/30 flex items-center justify-center">
            <svg className="w-10 h-10 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
          <div className="text-center">
            <h1 className="text-3xl font-bold text-white mb-2">Something went wrong</h1>
            <p className="text-rp-grey">The kiosk encountered an error</p>
          </div>
          <button
            onClick={() => window.location.reload()}
            className="px-8 py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-lg transition-colors"
          >
            Tap to Reload
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
