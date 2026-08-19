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

export function useUpdateOptional() {
  return useContext(UpdateContext);
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
    setError(null);
    try {
      const result = await api.installAppUpdate();
      if (!result.ok) {
        setError(result.error || "update failed");
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
  }, [update]);

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
    let unlistenMenu: (() => void) | undefined;
    let unlistenProgress: (() => void) | undefined;
    void import("@tauri-apps/api/event").then(({ listen }) => {
      if (cancelled) return;
      void listen("menu-check-update", () => {
        void check();
      }).then((fn) => {
        unlistenMenu = fn;
      });
      void listen<{ downloaded?: number; total?: number }>("update-progress", (event) => {
        const { downloaded, total } = event.payload ?? {};
        if (typeof downloaded === "number" && typeof total === "number" && total > 0) {
          setProgress(Math.min(100, Math.round((downloaded / total) * 100)));
        }
      }).then((fn) => {
        unlistenProgress = fn;
      });
    });
    return () => {
      cancelled = true;
      unlistenMenu?.();
      unlistenProgress?.();
    };
  }, [check]);

  const value = useMemo<UpdateContextValue>(
    () => ({ update, checking, installing, progress, error, check, install }),
    [update, checking, installing, progress, error, check, install],
  );

  return <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>;
}

export function UpdateBadge({ onOpen }: { onOpen: () => void }) {
  const ctx = useUpdateOptional();
  if (!ctx?.update?.available) return null;
  return (
    <button
      type="button"
      data-testid="update-badge"
      onClick={onOpen}
      className="inline-flex h-7 items-center rounded-full bg-primary px-2.5 text-xs font-medium text-primary-foreground"
    >
      {ctx.update.latestVersion ?? "Update"}
    </button>
  );
}
