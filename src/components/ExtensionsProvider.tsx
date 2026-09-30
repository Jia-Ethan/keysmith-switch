import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import * as api from "../api";
import type { ExtensionReport, ExtensionsView } from "../types";

export const EXT_FIRST_LOOK_DELAY_MS = 2500;
/** A running app looks again this often, and when the window returns after this long. */
export const EXT_RECHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;
export const EXT_RECHECK_ON_FOCUS_AFTER_MS = 60 * 60 * 1000;

/** The backend names a failure "extensions:<code>:<detail>"; only the code matters here. */
export function extensionErrorCode(error: unknown): string {
  const text = error instanceof Error ? error.message : String(error ?? "");
  return /extensions:([a-z-]+):/.exec(text)?.[1] ?? "default";
}

interface ExtensionsContextValue {
  enabled: boolean;
  view: ExtensionsView | null;
  /** Packs with a newer version to install; 0 whenever extensions are off. */
  updates: number;
  refreshing: boolean;
  /** The pack being installed or removed. */
  busyId: string | null;
  setEnabled: (next: boolean) => Promise<void>;
  refresh: () => Promise<void>;
  install: (id: string) => Promise<ExtensionReport>;
  uninstall: (id: string) => Promise<ExtensionReport>;
}

const ExtensionsContext = createContext<ExtensionsContextValue | null>(null);

export function useExtensions() {
  const value = useContext(ExtensionsContext);
  if (!value) throw new Error("useExtensions requires ExtensionsProvider");
  return value;
}

export function useExtensionsOptional() {
  return useContext(ExtensionsContext);
}

/**
 * Holds what the Extensions page and the rail badge both need. Nothing here reads the
 * network while `enabled` is false: the cached list is a local file, and every look
 * outward waits until the person has switched extensions on.
 */
export function ExtensionsProvider({
  enabled,
  onEnabledChange,
  children,
}: {
  enabled: boolean;
  onEnabledChange: (next: boolean) => Promise<void>;
  children: ReactNode;
}) {
  const [view, setView] = useState<ExtensionsView | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const lastLookRef = useRef<number | null>(null);
  const refreshingRef = useRef(false);

  const look = useCallback(async (silent: boolean) => {
    if (refreshingRef.current) return;
    refreshingRef.current = true;
    if (!silent) setRefreshing(true);
    try {
      setView(await api.extensionsRefresh());
      lastLookRef.current = Date.now();
    } catch (error) {
      // A person who asked sees the failure as an error state; a background look says nothing.
      if (!silent) setView((current) => (current ? { ...current, error: extensionErrorCode(error) } : current));
    } finally {
      refreshingRef.current = false;
      if (!silent) setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    if (!enabled) {
      setView(null);
      return;
    }
    let cancelled = false;
    void api
      .extensionsState()
      .then((cached) => {
        if (!cancelled) setView((current) => current ?? cached);
      })
      .catch(() => undefined);
    const first = window.setTimeout(() => void look(true), EXT_FIRST_LOOK_DELAY_MS);
    const timer = window.setInterval(() => void look(true), EXT_RECHECK_INTERVAL_MS);
    const onFocus = () => {
      const last = lastLookRef.current;
      if (last !== null && Date.now() - last >= EXT_RECHECK_ON_FOCUS_AFTER_MS) void look(true);
    };
    window.addEventListener("focus", onFocus);
    return () => {
      cancelled = true;
      window.clearTimeout(first);
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [enabled, look]);

  const setEnabled = useCallback(
    async (next: boolean) => {
      await onEnabledChange(next);
    },
    [onEnabledChange],
  );

  const refresh = useCallback(() => look(false), [look]);

  const change = useCallback(async (id: string, run: () => Promise<{ view: ExtensionsView; report: ExtensionReport }>) => {
    setBusyId(id);
    try {
      const result = await run();
      setView(result.view);
      return result.report;
    } finally {
      setBusyId(null);
    }
  }, []);

  const install = useCallback((id: string) => change(id, () => api.installExtension(id)), [change]);
  const uninstall = useCallback((id: string) => change(id, () => api.uninstallExtension(id)), [change]);

  const value = useMemo<ExtensionsContextValue>(
    () => ({
      enabled,
      view,
      updates: enabled ? view?.updates ?? 0 : 0,
      refreshing,
      busyId,
      setEnabled,
      refresh,
      install,
      uninstall,
    }),
    [enabled, view, refreshing, busyId, setEnabled, refresh, install, uninstall],
  );

  return <ExtensionsContext.Provider value={value}>{children}</ExtensionsContext.Provider>;
}
