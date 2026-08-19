import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { ToolId } from "../types";
import { TOOL_IDS } from "../types";
import { Button, cx } from "./ui";

export type AppPage =
  | { kind: "tool"; tool: ToolId }
  | { kind: "settings" }
  | { kind: "about" }
  | { kind: "advanced" };

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

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-11 shrink-0 items-center gap-3 border-b border-ink-200/10 bg-ink-800 px-3">
        <div className="min-w-[148px]">
          <div className="text-[13px] font-semibold tracking-tight">{t("app.name")}</div>
          <div className="text-[10px] text-ink-200">{t("app.workflow")}</div>
        </div>
        <nav className="flex flex-1 items-center gap-1" aria-label="tools">
          {TOOL_IDS.map((tool) => {
            const active = page.kind === "tool" && page.tool === tool;
            return (
              <Button
                key={tool}
                variant={active ? "primary" : "ghost"}
                data-testid={`nav-${tool}`}
                aria-current={active ? "page" : undefined}
                onClick={() => onNavigate({ kind: "tool", tool })}
              >
                {t(`nav.${tool}`)}
              </Button>
            );
          })}
        </nav>
        <div className="flex items-center gap-1">
          {advancedEnabled ? (
            <Button
              variant={page.kind === "advanced" ? "primary" : "ghost"}
              data-testid="nav-advanced"
              onClick={() => onNavigate({ kind: "advanced" })}
            >
              {t("nav.advanced")}
            </Button>
          ) : null}
          <Button
            variant={page.kind === "settings" ? "primary" : "ghost"}
            data-testid="nav-settings"
            onClick={() => onNavigate({ kind: "settings" })}
          >
            {t("nav.settings")}
          </Button>
          <Button
            variant={page.kind === "about" ? "primary" : "ghost"}
            data-testid="nav-about"
            onClick={() => onNavigate({ kind: "about" })}
          >
            {t("nav.about")}
          </Button>
        </div>
      </header>
      <main className={cx("min-h-0 flex-1 overflow-auto p-3")}>{children}</main>
    </div>
  );
}
