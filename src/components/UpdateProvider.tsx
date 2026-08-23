import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import * as api from "../api";
import { isTauriRuntime } from "../lib/runtime";
import type { UpdateChannel, UpdateCheck, UpdateInstall } from "../types";
import { PUBLIC_RELEASE_PAGE } from "../types";

export const AUTO_CHECK_DELAY_MS = 1800;

interface UpdateContextValue {
  update: UpdateCheck | null;
  checking: boolean;
  installing: boolean;
  progress: number | null;
  error: string | null;
  check: () => Promise<void>;
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

export function UpdateProvider({
  channel,
  autoCheck,
  children,
}: {
  channel: UpdateChannel;
  autoCheck: boolean;
  children: ReactNode;
}) {
  const [update, setUpdate] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const check = useCallback(async () => {
    setChecking(true);
    setProgress(null);
    setError(null);
    try {
      const result = await api.checkAppUpdate(channel);
      setUpdate(result);
      if (result.error) setError(result.error);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setChecking(false);
    }
  }, [channel]);

  const install = useCallback(async () => {
    if (!update?.available) return null;
    setInstalling(true);
    setProgress(0);
    setError(null);
    try {
      const result = await api.installAppUpdate(channel);
      if (!result.ok) {
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
      } satisfies UpdateInstall;
    } finally {
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
    () => ({ update, checking, installing, progress, error, check, install }),
    [update, checking, installing, progress, error, check, install],
  );

  return <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>;
}
