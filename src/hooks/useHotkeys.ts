import { useEffect, useRef } from "react";
import { isModKey } from "../lib/platform";

export interface Hotkey {
  /** KeyboardEvent.key, compared case-insensitively. */
  key: string;
  /** Require ⌘ (macOS) / Ctrl (elsewhere). */
  mod?: boolean;
  handler: (event: KeyboardEvent) => void;
  /** Also fire while focus is in a text field. Mod shortcuts default to true. */
  allowInInput?: boolean;
}

function isTextTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  return element.tagName === "INPUT" || element.tagName === "TEXTAREA" || element.isContentEditable;
}

/**
 * Window-level shortcuts. Ignored while a modal dialog is open unless the
 * caller lives inside it, so a shortcut can never act behind a confirmation.
 */
export function useHotkeys(hotkeys: Hotkey[], enabled = true): void {
  const ref = useRef(hotkeys);
  ref.current = hotkeys;

  useEffect(() => {
    if (!enabled) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.isComposing) return;
      for (const hotkey of ref.current) {
        if (event.key.toLowerCase() !== hotkey.key.toLowerCase()) continue;
        const mod = Boolean(hotkey.mod);
        if (mod !== isModKey(event)) continue;
        if (!mod && (event.metaKey || event.ctrlKey || event.altKey)) continue;
        const allowInInput = hotkey.allowInInput ?? mod;
        if (!allowInInput && isTextTarget(event.target)) continue;
        if (document.querySelector('[aria-modal="true"]')) continue;
        event.preventDefault();
        hotkey.handler(event);
        return;
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [enabled]);
}
