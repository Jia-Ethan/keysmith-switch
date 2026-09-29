// SPDX-License-Identifier: MIT
// Portions adapted from CC Switch (c) 2025 Jason Young
// https://github.com/farion1231/cc-switch
// Keysmith Switch: no framer-motion; Keysmith business only.

import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { Button } from "./ui";
import { IconChevronRight } from "./icons";
import { getFocusable, trapTab } from "../lib/focus";

export function FullScreenPanel({
  isOpen,
  title,
  onClose,
  children,
  footer,
  closeDisabled = false,
}: {
  isOpen: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  closeDisabled?: boolean;
}) {
  const { t } = useTranslation();
  const closeRef = useRef(onClose);
  const panelRef = useRef<HTMLDivElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  closeRef.current = onClose;

  useEffect(() => {
    if (!isOpen) return;
    previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (trapTab(event, panelRef.current)) return;
      if (event.key !== "Escape") return;
      if (closeDisabled) return;
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) {
        return;
      }
      event.stopPropagation();
      closeRef.current();
    };
    window.addEventListener("keydown", onKey);
    const initialFocus = getFocusable(panelRef.current)[0];
    if (initialFocus) initialFocus.focus();
    else panelRef.current?.focus();
    return () => {
      document.body.style.overflow = previous;
      window.removeEventListener("keydown", onKey);
      previousFocusRef.current?.focus();
    };
  }, [closeDisabled, isOpen]);

  if (!isOpen || typeof document === "undefined") return null;

  return createPortal(
    <div
      ref={panelRef}
      className="keysmith-surface animate-panel-in fixed inset-0 z-[60] flex flex-col bg-background"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      tabIndex={-1}
      data-testid="fullscreen-panel"
    >
      <header
        data-tauri-drag-region=""
        className="app-header glass flex h-[52px] shrink-0 items-center gap-3 border-b border-border/70 px-4"
      >
        <Button
          type="button"
          variant="outline"
          size="iconSm"
          aria-label={t("common.back")}
          title={`${t("common.back")} (Esc)`}
          data-testid="fullscreen-back"
          disabled={closeDisabled}
          onClick={onClose}
        >
          <IconChevronRight size={15} className="-scale-x-100" />
        </Button>
        <h2 data-tauri-drag-region="" className="min-w-0 flex-1 truncate text-[15px] font-semibold tracking-[-0.01em] text-foreground">
          {title}
        </h2>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-6 sm:px-6">{children}</div>
      {footer ? (
        <footer className="glass flex shrink-0 justify-end gap-3 border-t border-border/70 px-4 py-3 sm:px-6">
          {footer}
        </footer>
      ) : null}
    </div>,
    document.body,
  );
}
