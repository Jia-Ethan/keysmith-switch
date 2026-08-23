import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { Button, IconButton, cx } from "./ui";
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
  busy = false,
  wide = false,
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
  busy?: boolean;
  wide?: boolean;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useEffect(() => {
    if (!open) return;
    previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        if (!busy) closeRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      event.stopPropagation();
      const focusable = getFocusable(dialogRef.current);
      if (focusable.length === 0) {
        event.preventDefault();
        dialogRef.current?.focus();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && (document.activeElement === first || document.activeElement === dialogRef.current)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    const initialFocus = getFocusable(dialogRef.current)[0];
    if (initialFocus) initialFocus.focus();
    else dialogRef.current?.focus();
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      previousFocusRef.current?.focus();
    };
  }, [busy, open]);

  if (!open) return null;

  return createPortal(
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/45 p-4 backdrop-blur-sm">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        aria-busy={busy || undefined}
        tabIndex={-1}
        className={cx(
          "animate-dialog-in flex max-h-[86vh] w-full flex-col overflow-hidden rounded-xl border border-border bg-card shadow-[0_16px_60px_hsl(var(--shadow)/0.18)] focus:outline-none",
          wide ? "max-w-[560px]" : "max-w-[440px]",
        )}
      >
        <div className="flex shrink-0 items-start gap-3 border-b border-border px-5 py-3.5">
          <div className="min-w-0 flex-1">
            <h2 className="text-[15px] font-semibold text-foreground">{title}</h2>
            {description ? (
              <p className="mt-1 text-[13px] text-muted-foreground">{description}</p>
            ) : null}
          </div>
          <IconButton label={closeLabel} disabled={busy} onClick={onClose}>
            <IconClose />
          </IconButton>
        </div>
        <div className="min-h-0 flex-1 overflow-auto px-5 py-3.5 text-[13px]">{children}</div>
        <div className="flex shrink-0 justify-end gap-2 border-t border-border px-5 py-3.5">
          <Button disabled={busy} onClick={onClose}>{cancelLabel}</Button>
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
    </div>,
    document.body,
  );
}

function getFocusable(root: HTMLElement | null): HTMLElement[] {
  if (!root) return [];
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((element) => !element.hasAttribute("hidden") && element.getAttribute("aria-hidden") !== "true");
}
