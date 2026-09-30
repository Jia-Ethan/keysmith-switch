import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import * as api from "../api";
import { isTauriRuntime } from "../lib/runtime";
import type { UpdateChannel, UpdateCheck, UpdateInstall } from "../types";
import { PUBLIC_RELEASE_PAGE } from "../types";

export const AUTO_CHECK_DELAY_MS = 1800;
/**
 * A check against a fast feed answers in a few milliseconds, too quickly for the
 * person to see that anything happened. The checking state is held for at least
 * this long so the click always reads as an action.
 */
export const MIN_CHECK_MS = 700;
/** A running app looks again for a release this often, and when it regains focus after this long. */
export const RECHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;
export const RECHECK_ON_FOCUS_AFTER_MS = 60 * 60 * 1000;

interface UpdateContextValue {
  update: UpdateCheck | null;
  /**
   * A newer release is known. Unlike `update`, it survives a new check being under
   * way, so a badge does not blink out and back every time someone looks again.
   */
  hasUpdate: boolean;
  checking: boolean;
  installing: boolean;
  progress: number | null;
  error: string | null;
  checkCount: number;
  /** When the last check finished (any kind), or null before the first. */
  lastCheckedAt: number | null;
  /**
   * `silent` looks in the background: it never shows the checking state, never
   * clears what is known and never reports a failure. It only ever adds news.
   */
  check: (options?: { silent?: boolean }) => Promise<void>;
  install: () => Promise<UpdateInstall | null>;
}

const UpdateContext = createContext<UpdateContextValue | null>(null);

export function useUpdate() {
  const value = useContext(UpdateContext);
  if (!value) {
    throw new Error("useUpdate requires UpdateProvider");
  }
  return value;
}

export function useUpdateOptional() {
  return useContext(UpdateContext);
}

export function UpdateProvider({
  channel,
  autoCheck,
  minCheckMs = MIN_CHECK_MS,
  children,
}: {
  channel: UpdateChannel;
  autoCheck: boolean;
  /** Shortest time the checking state is shown; tests pass 0. */
  minCheckMs?: number;
  children: ReactNode;
}) {
  const [update, setUpdate] = useState<UpdateCheck | null>(null);
  const [hasUpdate, setHasUpdate] = useState(false);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checkCount, setCheckCount] = useState(0);
  const [lastCheckedAt, setLastCheckedAt] = useState<number | null>(null);
  const lastCheckedRef = useRef<number | null>(null);
  const checkingRef = useRef(false);
  const installingRef = useRef(false);

  const check = useCallback(async (options?: { silent?: boolean }) => {
    if (checkingRef.current || installingRef.current) return;
    if (options?.silent) {
      try {
        const result = await api.checkAppUpdate(channel);
        // A person's own check, or an install, started meanwhile and owns the state.
        if (checkingRef.current || installingRef.current || result.error) return;
        const now = Date.now();
        lastCheckedRef.current = now;
        setLastCheckedAt(now);
        setUpdate(result);
        setHasUpdate(result.available);
      } catch {
        // Nobody asked; a failed look is not news.
      }
      return;
    }
    checkingRef.current = true;
    const startedAt = Date.now();
    setChecking(true);
    setProgress(null);
    setError(null);
    setUpdate(null);
    try {
      const result = await api.checkAppUpdate(channel);
      setUpdate(result);
      if (result.error) {
        setError(result.error);
      } else {
        setHasUpdate(result.available);
      }
    } catch (err) {
      setUpdate(null);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      const remaining = minCheckMs - (Date.now() - startedAt);
      if (remaining > 0) await new Promise<void>((resolve) => window.setTimeout(resolve, remaining));
      const now = Date.now();
      lastCheckedRef.current = now;
      setLastCheckedAt(now);
      setCheckCount((count) => count + 1);
      checkingRef.current = false;
      setChecking(false);
    }
  }, [channel, minCheckMs]);

  const install = useCallback(async () => {
    if (!update?.available || update.installMode === "manual" || installingRef.current) return null;
    installingRef.current = true;
    setInstalling(true);
    setProgress(0);
    setError(null);
    try {
      const result = await api.installAppUpdate(channel);
      if (result.installMode === "manual") {
        setUpdate((current) => current ? {
          ...current,
          available: true,
          installMode: "manual",
          reason: result.reason,
          detail: result.detail ?? current.detail ?? null,
          restartRequired: false,
          error: null,
          releasePage: result.releasePage || current.releasePage,
        } : current);
        setProgress(null);
        setError(null);
      } else if (!result.ok) {
        setError(result.error || "update failed");
      } else {
        setProgress(100);
      }
      return result;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      return {
        ok: false,
        restartRequired: false,
        error: message,
        releasePage: PUBLIC_RELEASE_PAGE,
        installMode: "none",
        reason: null,
      } satisfies UpdateInstall;
    } finally {
      installingRef.current = false;
      setInstalling(false);
    }
  }, [channel, update]);

  useEffect(() => {
    if (!autoCheck) return;
    const timer = window.setTimeout(() => {
      void check();
    }, AUTO_CHECK_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [autoCheck, check]);

  // A window left open for days would never hear of a release: look again now and then.
  useEffect(() => {
    if (!autoCheck) return;
    const timer = window.setInterval(() => void check({ silent: true }), RECHECK_INTERVAL_MS);
    const onFocus = () => {
      const last = lastCheckedRef.current;
      if (last !== null && Date.now() - last >= RECHECK_ON_FOCUS_AFTER_MS) void check({ silent: true });
    };
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [autoCheck, check]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let cancelled = false;
    let unlistenProgress: (() => void) | undefined;
    void import("@tauri-apps/api/event").then(({ listen }) => {
      if (cancelled) return;
      void listen<{ downloaded?: number; total?: number; phase?: string }>("update-progress", (event) => {
        const { downloaded, total, phase } = event.payload ?? {};
        if (typeof downloaded === "number" && typeof total === "number" && total > 0) {
          setProgress(Math.min(100, Math.round((downloaded / total) * 100)));
        } else if (phase === "install") {
          setProgress(100);
        }
      }).then((fn) => {
        unlistenProgress = fn;
      });
    });
    return () => {
      cancelled = true;
      unlistenProgress?.();
    };
  }, []);

  const value = useMemo<UpdateContextValue>(
    () => ({ update, hasUpdate, checking, installing, progress, error, checkCount, lastCheckedAt, check, install }),
    [update, hasUpdate, checking, installing, progress, error, checkCount, lastCheckedAt, check, install],
  );

  return <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>;
}
