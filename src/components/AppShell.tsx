import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useHarnessStatus } from "../lib/harnessState";
import { shortcutLabel } from "../lib/platform";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { ScopeId, ToolId } from "../types";
import { TOOL_IDS } from "../types";
import { Button, cx, useSlidingIndicator } from "./ui";
import { IconMore, IconSettings } from "./icons";
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
  | { kind: "advanced" };

const NAV_GAP_PX = 2;
const NAV_PADDING_PX = 6;
const NAV_ITEM_FALLBACK_WIDTH = 88;
const NAV_MORE_FALLBACK_WIDTH = 48;
const NAV_BUTTON_CLASS =
  "relative z-[1] inline-flex h-8 items-center gap-2 rounded-[9px] px-3 text-[13px] font-medium transition-colors";
const NAV_MORE_CLASS =
  "relative z-[1] inline-flex h-8 items-center justify-center rounded-[9px] px-2.5 transition-colors";

/** Pack tool buttons into the centered nav slot, reserving space for overflow. */
export function countVisibleNavItems(
  available: number,
  itemWidths: number[],
  moreWidth: number,
  gap = NAV_GAP_PX,
  padding = NAV_PADDING_PX,
): number {
  const total = itemWidths.length;
  if (total === 0) return 0;
  if (available <= 0) return total;

  const packedWidth = (count: number, withMore: boolean) => {
    if (count <= 0) return padding + (withMore ? moreWidth : 0);
    const widths = itemWidths.slice(0, count);
    if (withMore && count < total) {
      widths[count - 1] = Math.max(...itemWidths.slice(count - 1));
    }
    const items = widths.reduce((sum, width) => sum + width, 0);
    const slots = count + (withMore ? 1 : 0);
    return padding + items + Math.max(0, slots - 1) * gap + (withMore ? moreWidth : 0);
  };

  if (packedWidth(total, false) <= available) return total;
  for (let count = total - 1; count >= 1; count -= 1) {
    if (packedWidth(count, true) <= available) return count;
  }
  return 1;
}

export function AppShell({
  page,
  onNavigate,
  advancedEnabled,
  children,
}: {
  page: AppPage;
  onNavigate: (page: AppPage) => void;
  advancedEnabled: boolean;
  children: ReactNode;
}) {
  const { t } = useTranslation();
  const navSlotRef = useRef<HTMLDivElement>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const moreRef = useRef<HTMLDivElement>(null);
  const [visibleCount, setVisibleCount] = useState(TOOL_IDS.length);
  const [moreOpen, setMoreOpen] = useState(false);

  useLayoutEffect(() => {
    const slot = navSlotRef.current;
    const measure = measureRef.current;
    if (!slot) return;

    const readWidth = (id: string, fallback: number) => {
      const el = measure?.querySelector<HTMLElement>(`[data-nav-measure="${id}"]`);
      const width = el?.offsetWidth ?? 0;
      return width > 0 ? width : fallback;
    };

    const compute = () => {
      const available = slot.clientWidth;
      if (available <= 0) {
        setVisibleCount(TOOL_IDS.length);
        return;
      }
      const itemWidths = TOOL_IDS.map((tool) => readWidth(tool, NAV_ITEM_FALLBACK_WIDTH));
      const moreWidth = readWidth("more", NAV_MORE_FALLBACK_WIDTH);
      const next = countVisibleNavItems(available, itemWidths, moreWidth);
      setVisibleCount((current) => (current === next ? current : next));
    };

    compute();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(compute);
    observer.observe(slot);
    if (measure) observer.observe(measure);
    window.addEventListener("resize", compute);
    const fonts = document.fonts;
    void fonts?.ready.then(compute);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", compute);
    };
  }, [t]);

  useEffect(() => {
    if (!moreOpen) return;
    const onPointerDown = (event: MouseEvent) => {
      if (!moreRef.current?.contains(event.target as Node)) setMoreOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMoreOpen(false);
    };
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [moreOpen]);

  const activeTool = page.kind === "tool" ? page.tool : null;
  const visible = TOOL_IDS.slice(0, visibleCount);
  if (activeTool && !visible.includes(activeTool)) {
    visible[visible.length - 1] = activeTool;
  }
  const overflow = TOOL_IDS.filter((tool) => !visible.includes(tool));
  const navRef = useRef<HTMLElement>(null);
  const indicator = useSlidingIndicator(navRef, '[aria-current="page"]', [activeTool, visible.join(",")]);

  const selectTool = (tool: ToolId) => {
    setMoreOpen(false);
    onNavigate({ kind: "tool", tool });
  };

  return (
    <div className="keysmith-surface relative flex h-full flex-col">
      <header
        data-tauri-drag-region=""
        className="app-header glass relative z-30 flex h-[52px] shrink-0 items-center gap-3 border-b border-border/70 px-4"
      >
        <div data-tauri-drag-region="" className="flex shrink-0 items-center gap-2.5">
          <span className="flex h-7 w-7 items-center justify-center rounded-lg bg-card shadow-[0_1px_2px_hsl(var(--shadow)/0.12)] ring-1 ring-border">
            <img src={keysmithIcon} alt="" className="h-5 w-5 shrink-0" aria-hidden="true" draggable={false} />
          </span>
          <div className="hidden whitespace-nowrap text-[14px] font-semibold tracking-[-0.01em] text-foreground sm:block">
            {t("app.name")}
          </div>
        </div>

        <div ref={navSlotRef} data-tauri-drag-region="" className="relative flex min-w-0 flex-1 items-center justify-center">
          <div
            ref={measureRef}
            aria-hidden="true"
            className="pointer-events-none invisible fixed left-0 top-0 flex items-center gap-0.5 overflow-hidden p-[3px]"
          >
            {TOOL_IDS.map((tool) => (
              <span key={tool} data-nav-measure={tool} className={NAV_BUTTON_CLASS}>
                <ToolLogo tool={tool} size={18} />
                <span className="hidden truncate lg:inline">{t(`nav.${tool}`)}</span>
              </span>
            ))}
            <span data-nav-measure="more" className={NAV_MORE_CLASS}>
              <IconMore size={16} />
            </span>
          </div>
          <nav
            ref={navRef}
            className="relative flex items-center gap-0.5 rounded-xl border border-border/80 bg-muted/70 p-[3px]"
            aria-label={t("nav.toolsLabel")}
          >
            {indicator ? (
              <span
                aria-hidden="true"
                data-testid="nav-indicator"
                className="pointer-events-none absolute rounded-[9px] bg-card shadow-[0_1px_2px_hsl(var(--shadow)/0.12),0_0_0_0.5px_hsl(var(--shadow)/0.08)] transition-[left,width] duration-[420ms] ease-[cubic-bezier(0.22,1,0.36,1)] dark:bg-accent"
                style={{ left: indicator.left, width: indicator.width, top: indicator.top, height: indicator.height }}
              >
                <span className="absolute inset-x-3 -bottom-px h-[2px] rounded-full bg-primary" />
              </span>
            ) : null}
            {visible.map((tool) => (
              <NavButton
                key={tool}
                tool={tool}
                active={activeTool === tool}
                indicatorReady={Boolean(indicator)}
                shortcut={shortcutLabel(String(TOOL_IDS.indexOf(tool) + 1))}
                onSelect={selectTool}
              />
            ))}
            {overflow.length > 0 ? (
              <div ref={moreRef} className="relative">
                <button
                  type="button"
                  data-testid="nav-more"
                  aria-haspopup="menu"
                  aria-expanded={moreOpen}
                  title={t("nav.more")}
                  aria-label={t("nav.more")}
                  onClick={() => setMoreOpen((value) => !value)}
                  className={cx(
                    NAV_MORE_CLASS,
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    moreOpen
                      ? "bg-card text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                >
                  <IconMore size={16} />
                </button>
                {moreOpen ? (
                  <div
                    role="menu"
                    className="animate-disclosure absolute right-0 top-11 z-30 w-52 overflow-hidden rounded-xl border border-border bg-card p-1 shadow-pop"
                  >
                    {overflow.map((tool) => (
                      <button
                        key={tool}
                        type="button"
                        role="menuitem"
                        data-testid={`nav-overflow-${tool}`}
                        onClick={() => selectTool(tool)}
                        className="flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[13px] text-foreground transition-colors hover:bg-muted"
                      >
                        <ToolLogo tool={tool} size={18} />
                        <span className="truncate">{t(`nav.${tool}`)}</span>
                      </button>
                    ))}
                  </div>
                ) : null}
              </div>
            ) : null}
          </nav>
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {advancedEnabled ? (
            <Button
              size="sm"
              variant={page.kind === "advanced" ? "subtle" : "ghost"}
              data-testid="nav-advanced"
              onClick={() => onNavigate({ kind: "advanced" })}
            >
              {t("nav.advanced")}
            </Button>
          ) : null}
          <Button
            size="iconSm"
            variant={page.kind === "settings" ? "subtle" : "ghost"}
            title={`${t("nav.settings")} (${shortcutLabel(",")})`}
            aria-label={t("nav.settings")}
            aria-current={page.kind === "settings" ? "page" : undefined}
            data-testid="nav-settings"
            onClick={() => onNavigate({ kind: "settings" })}
          >
            <IconSettings size={17} />
          </Button>
        </div>
      </header>

      <main className="min-h-0 flex-1 overflow-hidden">
        <div className="mx-auto flex h-full max-w-[1440px] flex-col">{children}</div>
      </main>
    </div>
  );
}

/** One agent in the switcher; the dot says this run already knows a prompt is live there. */
function NavButton({
  tool,
  active,
  indicatorReady,
  shortcut,
  onSelect,
}: {
  tool: ToolId;
  active: boolean;
  indicatorReady: boolean;
  shortcut: string;
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
      title={`${t(`nav.${tool}`)} (${shortcut})`}
      onClick={() => onSelect(tool)}
      className={cx(
        NAV_BUTTON_CLASS,
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        active
          ? cx("text-foreground", !indicatorReady && "bg-card shadow-sm")
          : "text-muted-foreground hover:bg-card/50 hover:text-foreground",
      )}
    >
      <span className={cx("relative transition-transform duration-300", active ? "scale-100" : "scale-[0.92] opacity-80 grayscale-[35%]")}>
        <ToolLogo tool={tool} size={18} />
        {deployed ? (
          <span
            className="absolute -right-1 -top-1 h-2 w-2 rounded-full bg-success ring-2 ring-muted"
            aria-label={t("status.deployed")}
          />
        ) : null}
      </span>
      <span className="hidden truncate lg:inline">{t(`nav.${tool}`)}</span>
    </button>
  );
}
