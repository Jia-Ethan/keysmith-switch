import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { ToolId } from "../types";
import { TOOL_IDS } from "../types";
import { Button, cx } from "./ui";
import { IconMore, IconSettings } from "./icons";
import { ToolLogo } from "./ToolLogos";
import { UpdateBadge } from "./UpdateProvider";

export type AppPage =
  | { kind: "tool"; tool: ToolId }
  | { kind: "settings"; tab?: string }
  | { kind: "advanced" };

const NAV_ITEM_MIN_WIDTH = 52;

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
  const moreRef = useRef<HTMLDivElement>(null);
  const [visibleCount, setVisibleCount] = useState(TOOL_IDS.length);
  const [moreOpen, setMoreOpen] = useState(false);

  useLayoutEffect(() => {
    const slot = navSlotRef.current;
    if (!slot || typeof ResizeObserver === "undefined") return;
    const compute = () => {
      const available = slot.clientWidth;
      if (available <= 0) {
        setVisibleCount(TOOL_IDS.length);
        return;
      }
      const total = TOOL_IDS.length;
      if (available >= total * NAV_ITEM_MIN_WIDTH) {
        setVisibleCount(total);
        return;
      }
      const fit = Math.floor((available - NAV_ITEM_MIN_WIDTH) / NAV_ITEM_MIN_WIDTH);
      setVisibleCount(Math.max(1, Math.min(total - 1, fit)));
    };
    compute();
    const observer = new ResizeObserver(compute);
    observer.observe(slot);
    return () => observer.disconnect();
  }, []);

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
    <div className="flex h-full flex-col bg-background">
      <header className="flex h-16 shrink-0 items-center gap-3 border-b border-border bg-card px-4">
        <div className="flex shrink-0 items-center gap-2">
          <span className="h-5 w-5 shrink-0 rounded bg-primary" aria-hidden="true" />
          <div className="leading-tight">
            <div className="whitespace-nowrap text-sm font-semibold tracking-tight text-foreground">
              {t("app.name")}
            </div>
            <div className="text-[11px] font-medium uppercase tracking-wide text-amber-700 dark:text-amber-400">
              {t("app.preview")}
            </div>
          </div>
        </div>

        <div ref={navSlotRef} className="flex min-w-0 flex-1 items-center justify-center">
          <nav
            className="flex items-center gap-1 rounded-xl bg-muted p-1"
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
                    "group inline-flex h-9 items-center gap-2 rounded-md px-3 text-sm font-medium transition-colors",
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
                    "inline-flex h-9 items-center justify-center rounded-md px-3 transition-colors",
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
                    className="absolute right-0 top-11 z-30 w-52 overflow-hidden rounded-md border border-border bg-card py-1 shadow-lg"
                  >
                    {overflow.map((tool) => (
                      <button
                        key={tool}
                        type="button"
                        role="menuitem"
                        data-testid={`nav-overflow-${tool}`}
                        onClick={() => selectTool(tool)}
                        className="flex w-full items-center gap-2.5 px-3 py-2 text-left text-sm text-foreground transition-colors hover:bg-muted"
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
          <UpdateBadge onOpen={() => onNavigate({ kind: "settings", tab: "about" })} />
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
            className="h-9 w-9"
            onClick={() => onNavigate({ kind: "settings" })}
          >
            <IconSettings size={16} />
          </Button>
        </div>
      </header>

      <main className="min-h-0 flex-1 overflow-hidden">
        <div className="mx-auto flex h-full max-w-[1440px] flex-col p-4">{children}</div>
      </main>
    </div>
  );
}
