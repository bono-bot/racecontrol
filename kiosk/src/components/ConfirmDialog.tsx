"use client";

import * as React from "react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

export type ConfirmVariant = "destructive" | "warning" | "default";

export interface ConfirmDialogState {
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: ConfirmVariant;
  onConfirm: () => void | Promise<void>;
}

interface ConfirmDialogProps {
  state: ConfirmDialogState | null;
  onDismiss: () => void;
}

export function ConfirmDialog({ state, onDismiss }: ConfirmDialogProps) {
  const [busy, setBusy] = React.useState(false);

  const open = state !== null;

  const handleConfirm = async () => {
    if (!state) return;
    try {
      setBusy(true);
      await state.onConfirm();
    } finally {
      setBusy(false);
      onDismiss();
    }
  };

  const handleOpenChange = (next: boolean) => {
    if (!next && !busy) onDismiss();
  };

  const confirmVariant: "default" | "destructive" | "outline" =
    state?.variant === "destructive" ? "destructive" : "default";

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>{state?.title ?? ""}</DialogTitle>
          {state?.description ? (
            <DialogDescription>{state.description}</DialogDescription>
          ) : null}
        </DialogHeader>
        <DialogFooter>
          <Button
            variant="outline"
            size="lg"
            onClick={onDismiss}
            disabled={busy}
          >
            {state?.cancelLabel ?? "Cancel"}
          </Button>
          <Button
            variant={confirmVariant}
            size="lg"
            onClick={handleConfirm}
            disabled={busy}
            autoFocus
          >
            {state?.confirmLabel ?? "Confirm"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
