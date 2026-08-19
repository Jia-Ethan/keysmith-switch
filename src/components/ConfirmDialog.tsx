import type { ReactNode } from "react";
import { Button } from "./ui";

export function ConfirmDialog({
  open,
  title,
  children,
  confirmLabel,
  cancelLabel,
  confirmDisabled,
  confirmTestId,
  onConfirm,
  onClose,
  danger,
}: {
  open: boolean;
  title: string;
  children: ReactNode;
  confirmLabel: string;
  cancelLabel: string;
  confirmDisabled?: boolean;
  confirmTestId?: string;
  onConfirm: () => void;
  onClose: () => void;
  danger?: boolean;
}) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/55 p-4">
      <div
        role="dialog"
        aria-modal="true"
        className="max-h-[86vh] w-full max-w-2xl overflow-auto rounded-lg border border-ink-200/15 bg-ink-900 p-4 shadow-xl"
      >
        <h2 className="mb-3 text-[15px] font-semibold">{title}</h2>
        <div className="mb-4 space-y-3 text-[12px]">{children}</div>
        <div className="flex justify-end gap-2">
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
