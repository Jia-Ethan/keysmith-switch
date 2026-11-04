import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useHarnessStatus } from "../lib/harnessState";
import type { ReactNode, RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { ScopeId, ToolId } from "../types";
import { TOOL_IDS } from "../types";
import { CountBadge, NoticeDot, cx, useSlidingIndicator } from "./ui";
import { IconBell, IconPuzzle, IconSearch, IconSettings, IconSwap, IconTerminal } from "./icons";
import { paletteShortcutLabel } from "./CommandPalette";
import { useAnnouncementsOptional } from "./AnnouncementsProvider";
import { useExtensionsOptional } from "./ExtensionsProvider";
import { useUpdateOptional } from "./UpdateProvider";
import { ToolLogo } from "./ToolLogos";
import keysmithIcon from "../assets/keysmith-icon.png";

export type AppPage =
  | { kind: "tool"; tool: ToolId }
  | { kind: "prompt-view"; tool: ToolId; promptId: string; scope: ScopeId; projectDir: string }
  | {
      kind: "prompt-edit";
      tool: ToolId;
      promptId?: string;
      creating: boolean;
      scope: ScopeId;
      projectDir: string;
    }
  | { kind: "settings"; tab?: string }
  | { kind: "extensions" }
  | { kind: "rewrite" }
  | { kind: "announcements" }
  | { kind: "advanced" };

const RAIL_BUTTON =
  "relative z-[1] flex flex-col items-center rounded-2xl text-center " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

/** How the agents sit on the rail: named, or closed up, and in the worst case scrolling. */
type RailFit = { compact: boolean; scrolls: boolean };

/**
 * Fits the rail into the window's height. With room, every agent shows its name. When the
 * window is too short the names fold away (they stay as each button's tooltip and accessible
 * name) and the rail closes up; only if even that does not fit does the agent list scroll.
 * The entries below the agents never shrink or scroll, so Settings stays in view at any
 * window height.
 *
 * When the names fold, the rail remembers how tall it needed to be to show them, and brings
 * them back only once it is that tall again, so it never flips back and forth.
 * `remeasureKey` changes when the names themselves change (another language).
 */
function useRailFit(
  rail: RefObject<HTMLElement | null>,
  list: RefObject<HTMLElement | null>,
  remeasureKey: unknown,
): RailFit {
  const [fit, setFit] = useState<RailFit>({ compact: false, scrolls: false });
  const fitRef = useRef(fit);
  const namedNeed = useRef(0);

  const measure = useCallback(() => {
    const outer = rail.current;
    const inner = list.current;
    if (!outer || !inner) return;
    const current = fitRef.current;
    // A scrolling list carries padding for its fades; it would not be there without the scroll.
    const style = current.scrolls ? getComputedStyle(inner) : null;
    const padding = style ? (parseFloat(style.paddingTop) || 0) + (parseFloat(style.paddingBottom) || 0) : 0;
    const overflow = inner.scrollHeight - padding - inner.clientHeight;
    let next = current;
    if (!current.compact) {
      if (overflow > 0) {
        namedNeed.current = outer.clientHeight + overflow;
        next = { compact: true, scrolls: false };
      }
    } else if (outer.clientHeight >= namedNeed.current) {
      next = { compact: false, scrolls: false };
    } else {
      next = { compact: true, scrolls: overflow > 0 };
    }
    if (next.compact !== current.compact || next.scrolls !== current.scrolls) {
      fitRef.current = next;
      setFit(next);
    }
  }, [rail, list]);

  // After every switch, look again: folding the names may still not be enough.
  useLayoutEffect(measure, [measure, fit.compact, fit.scrolls]);

  // New names may be longer or shorter: forget the old need and measure with names shown.
  useLayoutEffect(() => {
    namedNeed.current = 0;
    measure();
  }, [measure, remeasureKey]);

  // The rail follows the window; the agents change size when a font arrives.
  useLayoutEffect(() => {
    const outer = rail.current;
    const inner = list.current;
    if (!outer || !inner || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(outer);
    for (const child of Array.from(inner.children)) observer.observe(child);
    return () => observer.disconnect();
  }, [rail, list, measure]);

  return fit;
}

/**
 * The app frame: a slim rail of agents on the left, the workspace on the right.
 * The rail is the only navigation; there is no top bar to compete with the content.
 */
export function AppShell({
  page,
  onNavigate,
  advancedEnabled,
  onOpenPalette,
  children,
}: {
  page: AppPage;
  onNavigate: (page: AppPage) => void;
  advancedEnabled: boolean;
  /** Opens the command palette; the rail shows its button only when this is given. */
  onOpenPalette?: () => void;
  children: ReactNode;
}) {
  const { t, i18n } = useTranslation();
  const hasUpdate = Boolean(useUpdateOptional()?.hasUpdate);
  const extensions = useExtensionsOptional();
  const announcements = useAnnouncementsOptional();
  const railRef = useRef<HTMLElement>(null);
  const agentsRef = useRef<HTMLDivElement>(null);
  const activeTool = page.kind === "tool" ? page.tool : null;
  const railKey = page.kind === "tool" ? page.tool : page.kind;
  const fit = useRailFit(railRef, agentsRef, i18n.language);
  const measured = useSlidingIndicator(railRef, '[aria-current="page"]', [
    railKey,
    fit.compact,
    fit.scrolls,
    advancedEnabled,
    Boolean(extensions),
    Boolean(announcements),
  ]);
  // The halo is measured against the whole rail, so it cannot follow an agent through a
  // scrolled list; there the active agent wears its own background instead.
  const indicator = fit.scrolls && activeTool ? null : measured;

  useEffect(() => {
    if (!fit.scrolls || !activeTool) return;
    agentsRef.current?.querySelector<HTMLElement>('[aria-current="page"]')?.scrollIntoView?.({ block: "nearest" });
  }, [fit.scrolls, activeTool]);

  return (
    <div className="keysmith-surface relative flex h-full">
      <aside
        data-tauri-drag-region=""
        className="app-rail glass relative z-30 flex w-[88px] shrink-0 flex-col items-center border-r border-border/70 pb-3"
      >
        <button
          type="button"
          data-testid="nav-brand"
          title={t("nav.about")}
          aria-label={t("nav.about")}
          onClick={() => onNavigate({ kind: "settings", tab: "about" })}
          className="brand-mark mb-4 shrink-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <img src={keysmithIcon} alt="" className="h-11 w-11" draggable={false} />
        </button>

        <nav
          ref={railRef}
          aria-label={t("nav.toolsLabel")}
          className="relative flex min-h-0 w-full flex-1 flex-col items-center gap-1"
        >
          {indicator ? (
            <span
              aria-hidden="true"
              data-testid="nav-indicator"
              className="rail-indicator pointer-events-none absolute rounded-2xl"
              style={{ left: indicator.left, width: indicator.width, top: indicator.top, height: indicator.height }}
            />
          ) : null}
          <div
            ref={agentsRef}
            data-testid="rail-agents"
            data-compact={fit.compact || undefined}
            data-scrolls={fit.scrolls || undefined}
            className={cx(
              "flex min-h-0 w-full flex-1 flex-col items-center",
              fit.compact ? "gap-0.5" : "gap-1",
              fit.scrolls && "rail-agents-scroll overflow-y-auto overflow-x-hidden",
            )}
          >
            {TOOL_IDS.map((tool) => (
              <RailButton
                key={tool}
                tool={tool}
                active={activeTool === tool}
                compact={fit.compact}
                indicatorReady={Boolean(indicator)}
                onSelect={(next) => onNavigate({ kind: "tool", tool: next })}
              />
            ))}
          </div>
          <div
            data-testid="rail-utilities"
            className={cx("flex shrink-0 flex-col items-center", fit.compact ? "gap-0.5" : "gap-1")}
          >
            {extensions ? (
              <UtilityButton
                compact={fit.compact}
                testId="nav-extensions"
                label={extensions.updates > 0 ? `${t("nav.extensions")} · ${t("nav.extensionsUpdates", { count: extensions.updates })}` : t("nav.extensions")}
                count={extensions.updates}
                active={page.kind === "extensions"}
                indicatorReady={Boolean(indicator)}
                onClick={() => onNavigate({ kind: "extensions" })}
              >
                <IconPuzzle size={19} />
              </UtilityButton>
            ) : null}
            <UtilityButton
              compact={fit.compact}
              testId="nav-rewrite"
              label={t("nav.rewrite")}
              active={page.kind === "rewrite"}
              indicatorReady={Boolean(indicator)}
              onClick={() => onNavigate({ kind: "rewrite" })}
            >
              <IconSwap size={19} />
            </UtilityButton>
            {onOpenPalette ? (
              <UtilityButton
                compact={fit.compact}
                testId="nav-palette"
                label={`${t("palette.open")} (${paletteShortcutLabel()})`}
                active={false}
                indicatorReady
                onClick={onOpenPalette}
              >
                <IconSearch size={18} />
              </UtilityButton>
            ) : null}
            {advancedEnabled ? (
              <UtilityButton
                compact={fit.compact}
                testId="nav-advanced"
                label={t("nav.advanced")}
                active={page.kind === "advanced"}
                indicatorReady={Boolean(indicator)}
                onClick={() => onNavigate({ kind: "advanced" })}
              >
                <IconTerminal size={19} />
              </UtilityButton>
            ) : null}
            {announcements ? (
              <UtilityButton
                compact={fit.compact}
                testId="nav-announcements"
                label={announcements.unread > 0 ? `${t("nav.announcements")} · ${t("nav.announcementsNew")}` : t("nav.announcements")}
                count={announcements.unread}
                active={page.kind === "announcements"}
                indicatorReady={Boolean(indicator)}
                onClick={() => onNavigate({ kind: "announcements" })}
              >
                <IconBell size={19} />
              </UtilityButton>
            ) : null}
            <UtilityButton
              compact={fit.compact}
              testId="nav-settings"
              label={hasUpdate ? `${t("nav.settings")} · ${t("nav.updateAvailable")}` : t("nav.settings")}
              badge={hasUpdate}
              active={page.kind === "settings"}
              indicatorReady={Boolean(indicator)}
              onClick={() => onNavigate({ kind: "settings" })}
            >
              <IconSettings size={19} />
            </UtilityButton>
          </div>
        </nav>
      </aside>

      <main className="relative min-h-0 min-w-0 flex-1 overflow-hidden">
        <div data-tauri-drag-region="" className="app-drag-strip absolute inset-x-0 top-0 z-20 h-8" />
        <div className="mx-auto flex h-full max-w-[1240px] flex-col">{children}</div>
      </main>
    </div>
  );
}

/**
 * One agent on the rail; the dot says this run already knows a prompt is live there.
 * Compact drops the visible name for a short window; the tooltip and accessible name keep it.
 */
function RailButton({
  tool,
  active,
  compact,
  indicatorReady,
  onSelect,
}: {
  tool: ToolId;
  active: boolean;
  compact: boolean;
  indicatorReady: boolean;
  onSelect: (tool: ToolId) => void;
}) {
  const { t } = useTranslation();
  const status = useHarnessStatus(tool);
  const deployed = status?.machine === "deployed" && !status.error;
  return (
    <button
      type="button"
      data-testid={`nav-${tool}`}
      data-deployed={deployed || undefined}
      aria-current={active ? "page" : undefined}
      title={t(`nav.${tool}`)}
      onClick={() => onSelect(tool)}
      className={cx(
        RAIL_BUTTON,
        compact ? "w-[56px] p-[3px]" : "w-[68px] gap-1 px-1 pb-1.5 pt-2",
        "group shrink-0 transition-colors duration-200",
        active ? cx("text-foreground", !indicatorReady && "bg-card shadow-sm") : "text-muted-foreground hover:text-foreground",
      )}
    >
      <span
        className={cx(
          "rail-tile relative flex h-11 w-11 items-center justify-center rounded-[14px]",
          active ? "rail-tile-active" : "group-hover:-translate-y-0.5 group-hover:bg-card/80 group-active:scale-95",
        )}
      >
        <ToolLogo tool={tool} size={24} />
        {deployed ? (
          <span
            className="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full bg-success ring-2 ring-background"
            aria-label={t("status.deployed")}
          />
        ) : null}
      </span>
      <span className={compact ? "sr-only" : "w-full text-[10.5px] font-medium leading-[1.15]"}>{t(`nav.${tool}`)}</span>
    </button>
  );
}

function UtilityButton({
  compact = false,
  testId,
  label,
  badge = false,
  count = 0,
  active,
  indicatorReady,
  onClick,
  children,
}: {
  /** A shade smaller, for a short window. */
  compact?: boolean;
  testId: string;
  label: string;
  /** A small red dot: something here wants a look. */
  badge?: boolean;
  /** A numeric corner badge; it wins over `badge` whenever it is above zero. */
  count?: number;
  active: boolean;
  indicatorReady: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      data-testid={testId}
      title={label}
      aria-label={label}
      aria-current={active ? "page" : undefined}
      onClick={onClick}
      className={cx(
        "relative z-[1] flex shrink-0 items-center justify-center transition-[color,background-color,transform] duration-200 active:scale-95",
        compact ? "h-10 w-10 rounded-[13px]" : "h-11 w-11 rounded-[14px]",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        active
          ? cx("text-foreground", !indicatorReady && "bg-card shadow-sm")
          : "text-muted-foreground hover:bg-card/70 hover:text-foreground",
      )}
    >
      {children}
      {count > 0 ? <CountBadge count={count} /> : badge ? <NoticeDot /> : null}
    </button>
  );
}
