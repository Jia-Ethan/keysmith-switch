// SPDX-License-Identifier: MIT
// Portions adapted from CC Switch (c) 2025 Jason Young
// https://github.com/farion1231/cc-switch
// Keysmith Switch: no framer-motion; Keysmith business only.

import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { Button } from "./ui";
import { IconChevronRight } from "./icons";

export function FullScreenPanel({
  isOpen,
  title,
  onClose,
  children,
  footer,
}: {
  isOpen: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useEffect(() => {
    if (!isOpen) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) {
        return;
      }
      event.stopPropagation();
      closeRef.current();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      document.body.style.overflow = previous;
      window.removeEventListener("keydown", onKey);
    };
  }, [isOpen]);

  if (!isOpen || typeof document === "undefined") return null;

  return createPortal(
    <div
      className="fixed inset-0 z-[60] flex flex-col bg-background"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      data-testid="fullscreen-panel"
    >
      <header className="flex h-16 shrink-0 items-center gap-4 border-b border-border px-6">
        <Button
          type="button"
          variant="outline"
          size="icon"
          aria-label="Back"
          data-testid="fullscreen-back"
          onClick={onClose}
          className="h-9 w-9"
        >
          <span className="-scale-x-100">
            <IconChevronRight size={16} />
          </span>
        </Button>
        <h2 className="truncate text-lg font-semibold text-foreground">{title}</h2>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-6">{children}</div>
      {footer ? (
        <footer className="flex shrink-0 justify-end gap-3 border-t border-border px-6 py-4">
          {footer}
        </footer>
      ) : null}
    </div>,
    document.body,
  );
}
