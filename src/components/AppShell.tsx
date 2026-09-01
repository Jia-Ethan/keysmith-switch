import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { ScopeId, ToolId } from "../types";
import { TOOL_IDS } from "../types";
import { Button, cx } from "./ui";
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

const NAV_GAP_PX = 4;
const NAV_PADDING_PX = 8;
const NAV_ITEM_FALLBACK_WIDTH = 88;
const NAV_MORE_FALLBACK_WIDTH = 48;
const NAV_BUTTON_CLASS =
  "inline-flex h-10 items-center gap-2 rounded-xl px-3 text-[15px] font-medium transition-colors";
const NAV_MORE_CLASS =
  "inline-flex h-10 items-center justify-center rounded-xl px-3 transition-colors";

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

  const selectTool = (tool: ToolId) => {
    setMoreOpen(false);
    onNavigate({ kind: "tool", tool });
  };

  return (
    <div className="keysmith-surface flex h-full flex-col">
      <header className="flex h-16 shrink-0 items-center gap-3 border-b border-border bg-card px-4 sm:px-5">
        <div className="flex shrink-0 items-center gap-2.5">
          <img src={keysmithIcon} alt="" className="h-7 w-7 shrink-0" aria-hidden="true" />
          <div className="whitespace-nowrap text-[16px] font-semibold tracking-tight text-foreground">
            {t("app.name")}
          </div>
        </div>

        <div ref={navSlotRef} className="relative flex min-w-0 flex-1 items-center justify-center">
          <div
            ref={measureRef}
            aria-hidden="true"
            className="pointer-events-none invisible fixed left-0 top-0 flex items-center gap-1 overflow-hidden p-1"
          >
            {TOOL_IDS.map((tool) => (
              <span key={tool} data-nav-measure={tool} className={NAV_BUTTON_CLASS}>
                <ToolLogo tool={tool} size={20} />
                <span className="hidden truncate xl:inline">{t(`nav.${tool}`)}</span>
              </span>
            ))}
            <span data-nav-measure="more" className={NAV_MORE_CLASS}>
              <IconMore size={16} />
            </span>
          </div>
          <nav
            className="flex items-center gap-1 rounded-2xl bg-muted p-1"
            aria-label={t("nav.toolsLabel")}
          >
            {visible.map((tool) => {
              const active = activeTool === tool;
              return (
                <button
                  key={tool}
                  type="button"
                  data-testid={`nav-${tool}`}
                  aria-current={active ? "page" : undefined}
                  title={t(`nav.${tool}`)}
                  onClick={() => selectTool(tool)}
                  className={cx(
                    NAV_BUTTON_CLASS,
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    active
                      ? "bg-background text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground hover:bg-background/50",
                  )}
                >
                  <ToolLogo tool={tool} size={20} />
                  <span className="hidden truncate xl:inline">{t(`nav.${tool}`)}</span>
                </button>
              );
            })}
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
                      ? "bg-background text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                >
                  <IconMore size={16} />
                </button>
                {moreOpen ? (
                  <div
                    role="menu"
                    className="absolute right-0 top-12 z-30 w-48 overflow-hidden rounded-xl border border-border bg-card py-1 shadow-lg"
                  >
                    {overflow.map((tool) => (
                      <button
                        key={tool}
                        type="button"
                        role="menuitem"
                        data-testid={`nav-overflow-${tool}`}
                        onClick={() => selectTool(tool)}
                        className="flex w-full items-center gap-2 px-3 py-2 text-left text-[15px] text-foreground transition-colors hover:bg-muted"
                      >
                        <ToolLogo tool={tool} size={20} />
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
              variant={page.kind === "advanced" ? "primary" : "ghost"}
              data-testid="nav-advanced"
              onClick={() => onNavigate({ kind: "advanced" })}
            >
              {t("nav.advanced")}
            </Button>
          ) : null}
          <Button
            size="icon"
            variant={page.kind === "settings" ? "primary" : "ghost"}
            title={t("nav.settings")}
            aria-label={t("nav.settings")}
            data-testid="nav-settings"
            onClick={() => onNavigate({ kind: "settings" })}
          >
            <IconSettings size={18} />
          </Button>
        </div>
      </header>

      <main className="min-h-0 flex-1 overflow-hidden">
        <div className="mx-auto flex h-full max-w-[1440px] flex-col p-4">{children}</div>
      </main>
    </div>
  );
}
