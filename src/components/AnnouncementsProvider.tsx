import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import * as api from "../api";
import type { AnnouncementsView } from "../types";

export const ANN_FIRST_LOOK_DELAY_MS = 3000;
/** A running app looks again this often, and when the window returns after this long. */
export const ANN_RECHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;
export const ANN_RECHECK_ON_FOCUS_AFTER_MS = 60 * 60 * 1000;

interface AnnouncementsContextValue {
  view: AnnouncementsView | null;
  unread: number;
  refreshing: boolean;
  refresh: () => Promise<void>;
  /** Opened in the list: no longer counted as new. */
  markRead: (ids: string[]) => Promise<void>;
  /** The pinned card was closed: it does not come back. */
  dismiss: (id: string) => Promise<void>;
}

const AnnouncementsContext = createContext<AnnouncementsContextValue | null>(null);

export function useAnnouncements() {
  const value = useContext(AnnouncementsContext);
  if (!value) throw new Error("useAnnouncements requires AnnouncementsProvider");
  return value;
}

export function useAnnouncementsOptional() {
  return useContext(AnnouncementsContext);
}

/**
 * Holds the announcements for the rail badge, the list and the pinned cards. The cached copy is
 * shown at once; the feed is read a little after start, then every few hours. A look that fails
 * in the background says nothing and keeps what is shown.
 */
export function AnnouncementsProvider({ children }: { children: ReactNode }) {
  const [view, setView] = useState<AnnouncementsView | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const lastLookRef = useRef<number | null>(null);
  const refreshingRef = useRef(false);

  const look = useCallback(async (silent: boolean) => {
    if (refreshingRef.current) return;
    refreshingRef.current = true;
    if (!silent) setRefreshing(true);
    try {
      setView(await api.announcementsRefresh());
      lastLookRef.current = Date.now();
    } catch {
      // Announcements never get in the way: a failed look keeps what is shown.
    } finally {
      refreshingRef.current = false;
      if (!silent) setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void api
      .announcementsState()
      .then((cached) => {
        if (!cancelled) setView((current) => current ?? cached);
      })
      .catch(() => undefined);
    const first = window.setTimeout(() => void look(true), ANN_FIRST_LOOK_DELAY_MS);
    const timer = window.setInterval(() => void look(true), ANN_RECHECK_INTERVAL_MS);
    const onFocus = () => {
      const last = lastLookRef.current;
      if (last !== null && Date.now() - last >= ANN_RECHECK_ON_FOCUS_AFTER_MS) void look(true);
    };
    window.addEventListener("focus", onFocus);
    return () => {
      cancelled = true;
      window.clearTimeout(first);
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [look]);

  const refresh = useCallback(() => look(false), [look]);

  const markRead = useCallback(async (ids: string[]) => {
    if (ids.length === 0) return;
    try {
      setView(await api.markAnnouncements(ids, false));
    } catch {
      // Not being able to remember a read mark is not worth an error.
    }
  }, []);

  const dismiss = useCallback(async (id: string) => {
    // Hide it at once; the backend keeps the mark for next time.
    setView((current) =>
      current
        ? { ...current, items: current.items.map((item) => (item.id === id ? { ...item, dismissed: true, read: true } : item)) }
        : current,
    );
    try {
      setView(await api.markAnnouncements([id], true));
    } catch {
      // The card stays hidden for this run either way.
    }
  }, []);

  const value = useMemo<AnnouncementsContextValue>(
    () => ({ view, unread: view?.unread ?? 0, refreshing, refresh, markRead, dismiss }),
    [view, refreshing, refresh, markRead, dismiss],
  );

  return <AnnouncementsContext.Provider value={value}>{children}</AnnouncementsContext.Provider>;
}
