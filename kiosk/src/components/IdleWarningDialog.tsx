'use client';

import React, { useEffect, useState } from 'react';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';

export interface IdleWarningPayload {
  pod_id: string;
  session_id: string;
  balance_paise: number;
  seconds_remaining: number;
  can_continue: boolean;
}

export interface IdleWarningDialogProps {
  warning: IdleWarningPayload | null;
  podNumber?: number;
  onContinue: (podId: string) => void;
  onEndSession: (sessionId: string) => void;
  onDismiss: () => void;
}

function formatCountdown(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
}

export function IdleWarningDialog({
  warning,
  podNumber,
  onContinue,
  onEndSession,
  onDismiss,
}: IdleWarningDialogProps): React.ReactElement | null {
  // Local 1s/sec interpolation. Reset whenever a new warning arrives (keyed by session_id).
  const [secondsLeft, setSecondsLeft] = useState<number>(warning?.seconds_remaining ?? 0);

  useEffect(() => {
    setSecondsLeft(warning?.seconds_remaining ?? 0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [warning?.session_id, warning?.seconds_remaining]);

  useEffect(() => {
    if (!warning || secondsLeft <= 0) return;
    const id = window.setInterval(() => {
      setSecondsLeft((prev) => Math.max(prev - 1, 0));
    }, 1000);
    return () => window.clearInterval(id);
  }, [warning, secondsLeft]);

  if (!warning) return null;

  const credits = Math.floor(warning.balance_paise / 100);
  const podLabel =
    podNumber !== undefined
      ? `POD ${String(podNumber).padStart(2, '0')}`
      : warning.pod_id.toUpperCase();

  // Countdown color: red <30s, amber 30-60s, white >60s
  const countdownColor =
    secondsLeft < 30
      ? 'text-rp-red'
      : secondsLeft < 60
      ? 'text-amber-500'
      : 'text-white';

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onDismiss();
      }}
    >
      <DialogContent className="bg-[#1A1A1A] border-rp-border p-8 max-w-md">
        <div className="flex flex-col items-center gap-4">
          {/* Pod label */}
          <span className="text-xs uppercase tracking-wider text-rp-grey w-full">
            {podLabel}
          </span>

          {warning.can_continue ? (
            // Branch A — has enough balance to continue
            <>
              <h2 className="text-xl font-bold text-white leading-tight w-full">
                Still here?
              </h2>

              {/* Countdown */}
              <span
                className={`text-3xl font-bold font-mono tabular-nums ${countdownColor}`}
                aria-live="polite"
              >
                {formatCountdown(secondsLeft)}
              </span>

              {/* Body */}
              <p className="text-sm text-white leading-relaxed w-full">
                Your session pauses at 15 minutes of inactivity. You have{' '}
                {secondsLeft}s left and {credits} credits remaining.
              </p>

              {/* CTAs */}
              <div className="flex flex-col w-full gap-3 mt-2">
                <Button
                  size="lg"
                  className="bg-rp-red hover:bg-rp-red-hover text-white font-semibold w-full"
                  autoFocus
                  onClick={() => {
                    onContinue(warning.pod_id);
                    onDismiss();
                  }}
                >
                  Tap to continue
                </Button>
                <Button
                  size="lg"
                  variant="outline"
                  className="border-rp-border text-rp-grey hover:bg-rp-surface w-full"
                  onClick={() => {
                    onDismiss();
                    onEndSession(warning.session_id);
                  }}
                >
                  End session now
                </Button>
              </div>
            </>
          ) : (
            // Branch B — out of credits
            <>
              <h2 className="text-xl font-bold text-white leading-tight w-full">
                Out of credits
              </h2>

              {/* Countdown — still shown so staff can see auto-end timing */}
              <span
                className={`text-3xl font-bold font-mono tabular-nums ${countdownColor}`}
                aria-live="polite"
              >
                {formatCountdown(secondsLeft)}
              </span>

              {/* Body */}
              <p className="text-sm text-white leading-relaxed w-full">
                Your wallet ({credits} credits) cannot cover another minute. Please top
                up at the POS to continue, or end the session now.
              </p>

              {/* Sole CTA */}
              <div className="flex flex-col w-full gap-3 mt-2">
                <Button
                  size="lg"
                  variant="outline"
                  className="border-2 border-rp-red text-rp-red hover:bg-rp-red hover:text-white font-semibold w-full"
                  autoFocus
                  onClick={() => {
                    onDismiss();
                    onEndSession(warning.session_id);
                  }}
                >
                  End session
                </Button>
              </div>
            </>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
