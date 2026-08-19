import { useCallback, useEffect, useState } from "react";

export type ThemeMode = "light" | "dark" | "system";

const STORAGE_KEY = "keysmith-switch-theme";

export function isThemeMode(value: unknown): value is ThemeMode {
  return value === "light" || value === "dark" || value === "system";
}

/**
 * Storage access is wrapped: a restricted WebView, private mode, or a disabled
 * storage partition throws on access. Theme is a cosmetic preference and must
 * never be able to take the whole app down.
 */
function readStored(): ThemeMode {
  try {
    const stored = window.localStorage?.getItem?.(STORAGE_KEY);
    return isThemeMode(stored) ? stored : "system";
  } catch {
    return "system";
  }
}

function writeStored(mode: ThemeMode): void {
  try {
    window.localStorage?.setItem?.(STORAGE_KEY, mode);
  } catch {
    // preference simply does not persist this session
  }
}

function prefersDark(): boolean {
  try {
    return Boolean(window.matchMedia?.("(prefers-color-scheme: dark)").matches);
  } catch {
    return false;
  }
}

export function resolveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "system") return prefersDark() ? "dark" : "light";
  return mode;
}

function applyTheme(mode: ThemeMode): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.classList.remove("light", "dark");
  root.classList.add(resolveTheme(mode));
}

/** Theme mode is a UI preference and stays in local storage, not in backend settings. */
export function useTheme() {
  const [theme, setThemeState] = useState<ThemeMode>(() =>
    typeof window === "undefined" ? "system" : readStored(),
  );

  useEffect(() => {
    applyTheme(theme);
    writeStored(theme);
  }, [theme]);

  useEffect(() => {
    if (theme !== "system" || typeof window === "undefined") return;
    let media: MediaQueryList;
    try {
      media = window.matchMedia("(prefers-color-scheme: dark)");
    } catch {
      return;
    }
    const onChange = () => applyTheme("system");
    media.addEventListener?.("change", onChange);
    return () => media.removeEventListener?.("change", onChange);
  }, [theme]);

  const setTheme = useCallback((next: ThemeMode) => {
    setThemeState(next);
  }, []);

  return { theme, setTheme, resolved: resolveTheme(theme) };
}
