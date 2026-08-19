import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import { Button, IconButton } from "./ui";
import { IconClose } from "./icons";

export function ConfirmDialog({
  open,
  title,
  description,
  children,
  confirmLabel,
  cancelLabel,
  confirmDisabled,
  confirmTestId,
  onConfirm,
  onClose,
  danger,
  closeLabel = "Close",
}: {
  open: boolean;
  title: string;
  description?: string;
  children: ReactNode;
  confirmLabel: string;
  cancelLabel: string;
  confirmDisabled?: boolean;
  confirmTestId?: string;
  onConfirm: () => void;
  onClose: () => void;
  danger?: boolean;
  closeLabel?: string;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKeyDown);
    dialogRef.current?.focus();
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose, open]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        className="flex max-h-[86vh] w-full max-w-2xl flex-col overflow-hidden rounded-xl border border-border bg-card shadow-xl focus:outline-none"
      >
        <div className="flex shrink-0 items-start gap-3 border-b border-border px-4 py-3">
          <div className="min-w-0 flex-1">
            <h2 className="text-[14px] font-semibold text-foreground">{title}</h2>
            {description ? (
              <p className="mt-0.5 text-[12px] text-muted-foreground">{description}</p>
            ) : null}
          </div>
          <IconButton label={closeLabel} onClick={onClose}>
            <IconClose />
          </IconButton>
        </div>
        <div className="min-h-0 flex-1 overflow-auto px-4 py-3 text-[12px]">{children}</div>
        <div className="flex shrink-0 justify-end gap-2 border-t border-border px-4 py-3">
          <Button onClick={onClose}>{cancelLabel}</Button>
          <Button
            variant={danger ? "danger" : "primary"}
            disabled={confirmDisabled}
            data-testid={confirmTestId}
            onClick={onConfirm}
          >
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
