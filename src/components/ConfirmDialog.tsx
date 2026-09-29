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
  icon,
  footerStart,
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
  /** Leading visual in the header, e.g. the agent logo. */
  icon?: ReactNode;
  /** Extra content on the left of the footer (secondary actions, hints). */
  footerStart?: ReactNode;
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
    <div className="fixed inset-0 z-[70] flex items-center justify-center p-4">
      <div
        className="animate-backdrop-in absolute inset-0 bg-[hsl(var(--shadow)/0.38)] backdrop-blur-[6px] dark:bg-black/55"
        aria-hidden="true"
        onMouseDown={() => {
          if (!busy) closeRef.current();
        }}
      />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        aria-busy={busy || undefined}
        tabIndex={-1}
        className={cx(
          "animate-dialog-in relative flex max-h-[86vh] w-full flex-col overflow-hidden rounded-[18px] border border-border bg-card focus:outline-none",
          "shadow-[0_0_0_1px_hsl(var(--shadow)/0.03),0_24px_80px_-12px_hsl(var(--shadow)/0.35)]",
          wide ? "max-w-[600px]" : "max-w-[440px]",
        )}
      >
        <div className="flex shrink-0 items-start gap-3 px-5 pb-3 pt-4">
          {icon ? (
            <div className="mt-0.5 flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 ring-1 ring-primary/15">
              {icon}
            </div>
          ) : null}
          <div className="min-w-0 flex-1">
            <h2 className="text-[16px] font-semibold tracking-[-0.01em] text-foreground">{title}</h2>
            {description ? (
              <p className="mt-0.5 truncate text-[12.5px] text-muted-foreground" title={description}>
                {description}
              </p>
            ) : null}
          </div>
          <IconButton label={closeLabel} size="iconSm" disabled={busy} onClick={onClose} className="-mr-1.5 -mt-0.5">
            <IconClose />
          </IconButton>
        </div>
        <div className="min-h-0 flex-1 overflow-auto px-5 pb-4 pt-1 text-[13.5px]">{children}</div>
        <div className="flex shrink-0 items-center gap-2 border-t border-border bg-muted/40 px-5 py-3">
          <div className="min-w-0 flex-1">{footerStart}</div>
          <Button disabled={busy} onClick={onClose}>{cancelLabel}</Button>
          <Button
            variant={danger ? "danger" : "primary"}
            disabled={confirmDisabled}
            data-testid={confirmTestId}
            onClick={onConfirm}
          >
            {busy ? <span className="harness-spinner" aria-hidden="true" /> : null}
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
