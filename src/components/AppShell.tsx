import { useRef } from "react";
import { useHarnessStatus } from "../lib/harnessState";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { ScopeId, ToolId } from "../types";
import { TOOL_IDS } from "../types";
import { cx, useSlidingIndicator } from "./ui";
import { IconSearch, IconSettings, IconTerminal } from "./icons";
import { paletteShortcutLabel } from "./CommandPalette";
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

const RAIL_BUTTON =
  "relative z-[1] flex w-[68px] flex-col items-center gap-1 rounded-2xl px-1 pb-1.5 pt-2 text-center " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

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
  const { t } = useTranslation();
  const railRef = useRef<HTMLElement>(null);
  const activeTool = page.kind === "tool" ? page.tool : null;
  const railKey = page.kind === "tool" ? page.tool : page.kind;
  const indicator = useSlidingIndicator(railRef, '[aria-current="page"]', [railKey]);

  return (
    <div className="keysmith-surface relative flex h-full">
      <aside
        data-tauri-drag-region=""
        className="app-rail glass relative z-30 flex w-[88px] shrink-0 flex-col items-center border-r border-border/70 pb-3"
      >
        <div data-tauri-drag-region="" className="mb-3 flex h-10 w-10 items-center justify-center rounded-xl bg-card shadow-[0_1px_2px_hsl(var(--shadow)/0.12)] ring-1 ring-border">
          <img src={keysmithIcon} alt={t("app.name")} className="h-6 w-6" draggable={false} />
        </div>

        <nav
          ref={railRef}
          aria-label={t("nav.toolsLabel")}
          className="relative flex min-h-0 flex-1 flex-col items-center gap-1"
        >
          {indicator ? (
            <span
              aria-hidden="true"
              data-testid="nav-indicator"
              className="rail-indicator pointer-events-none absolute rounded-2xl"
              style={{ left: indicator.left, width: indicator.width, top: indicator.top, height: indicator.height }}
            />
          ) : null}
          {TOOL_IDS.map((tool) => (
            <RailButton
              key={tool}
              tool={tool}
              active={activeTool === tool}
              indicatorReady={Boolean(indicator)}
              onSelect={(next) => onNavigate({ kind: "tool", tool: next })}
            />
          ))}
          <div className="mt-auto flex flex-col items-center gap-1">
            {onOpenPalette ? (
              <UtilityButton
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
                testId="nav-advanced"
                label={t("nav.advanced")}
                active={page.kind === "advanced"}
                indicatorReady={Boolean(indicator)}
                onClick={() => onNavigate({ kind: "advanced" })}
              >
                <IconTerminal size={19} />
              </UtilityButton>
            ) : null}
            <UtilityButton
              testId="nav-settings"
              label={t("nav.settings")}
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

/** One agent on the rail; the dot says this run already knows a prompt is live there. */
function RailButton({
  tool,
  active,
  indicatorReady,
  onSelect,
}: {
  tool: ToolId;
  active: boolean;
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
        "group transition-colors duration-200",
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
      <span className="w-full text-[10.5px] font-medium leading-[1.15]">{t(`nav.${tool}`)}</span>
    </button>
  );
}

function UtilityButton({
  testId,
  label,
  active,
  indicatorReady,
  onClick,
  children,
}: {
  testId: string;
  label: string;
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
        "relative z-[1] flex h-11 w-11 items-center justify-center rounded-[14px] transition-[color,background-color,transform] duration-200 active:scale-95",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        active
          ? cx("text-foreground", !indicatorReady && "bg-card shadow-sm")
          : "text-muted-foreground hover:bg-card/70 hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}
