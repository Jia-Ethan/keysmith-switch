import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { requestDeployPrompt, requestQuickDeploy } from "../lib/paletteEvents";
import { detectPlatform } from "../lib/platform";
import type { PromptSummary, ToolId } from "../types";
import { TOOL_IDS } from "../types";
import type { AppPage } from "./AppShell";
import { IconPlus, IconPuzzle, IconSwap, IconRocket, IconSearch, IconSettings } from "./icons";
import { ToolLogo } from "./ToolLogos";
import { cx } from "./ui";

interface Command {
  id: string;
  group: "actions" | "agents" | "prompts";
  label: string;
  hint?: string;
  icon: ReactNode;
  run: () => void;
}

export const paletteShortcutLabel = () => (detectPlatform() === "mac" ? "⌘K" : "Ctrl K");

/**
 * ⌘K: jump to an agent, start a Quick Deploy, or deploy a library prompt without
 * leaving the keyboard. Deploying still goes through the same review sheet.
 */
export function CommandPalette({
  open,
  onClose,
  page,
  onNavigate,
}: {
  open: boolean;
  onClose: () => void;
  page: AppPage;
  onNavigate: (page: AppPage) => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const [prompts, setPrompts] = useState<PromptSummary[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const currentTool: ToolId | null = page.kind === "tool" ? page.tool : null;

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setCursor(0);
    inputRef.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open || !currentTool) {
      setPrompts([]);
      return;
    }
    let cancelled = false;
    void api
      .listPrompts({ tool: currentTool, sort: "updated" })
      .then((result) => {
        if (!cancelled) setPrompts(result.prompts ?? []);
      })
      .catch(() => {
        if (!cancelled) setPrompts([]);
      });
    return () => {
      cancelled = true;
    };
  }, [open, currentTool]);

  const commands = useMemo<Command[]>(() => {
    const list: Command[] = [];
    // The workspace listens for these once this page has finished closing the palette.
    const afterClose = (action: () => void) => () => {
      onClose();
      window.setTimeout(action, 0);
    };
    if (currentTool) {
      list.push({
        id: "quick-deploy",
        group: "actions",
        label: t("palette.quickDeploy", { tool: t(`nav.${currentTool}`) }),
        icon: <IconPlus size={15} />,
        run: afterClose(requestQuickDeploy),
      });
    }
    for (const tool of TOOL_IDS) {
      if (tool === currentTool) continue;
      list.push({
        id: `agent-${tool}`,
        group: "agents",
        label: t("palette.goTo", { tool: t(`nav.${tool}`) }),
        icon: <ToolLogo tool={tool} size={16} />,
        run: () => {
          onClose();
          onNavigate({ kind: "tool", tool });
        },
      });
    }
    list.push({
      id: "extensions",
      group: "actions",
      label: t("nav.extensions"),
      icon: <IconPuzzle size={15} />,
      run: () => {
        onClose();
        onNavigate({ kind: "extensions" });
      },
    });
    list.push({
      id: "rewrite",
      group: "actions",
      label: t("nav.rewrite"),
      icon: <IconSwap size={15} />,
      run: () => {
        onClose();
        onNavigate({ kind: "rewrite" });
      },
    });
    list.push({
      id: "settings",
      group: "actions",
      label: t("nav.settings"),
      icon: <IconSettings size={15} />,
      run: () => {
        onClose();
        onNavigate({ kind: "settings" });
      },
    });
    for (const prompt of prompts) {
      list.push({
        id: `prompt-${prompt.id}`,
        group: "prompts",
        label: prompt.title,
        hint: t("palette.deployHint"),
        icon: <IconRocket size={15} />,
        run: afterClose(() => requestDeployPrompt(prompt.id)),
      });
    }
    return list;
  }, [currentTool, onClose, onNavigate, prompts, t]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return commands;
    return commands.filter((command) => command.label.toLowerCase().includes(needle));
  }, [commands, query]);

  useEffect(() => {
    setCursor((current) => Math.min(current, Math.max(0, visible.length - 1)));
  }, [visible.length]);

  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [cursor, visible]);

  if (!open || typeof document === "undefined") return null;

  const groups: Array<Command["group"]> = ["actions", "agents", "prompts"];
  const onKeyDown = (event: React.KeyboardEvent) => {
    // Enter while composing Chinese or Japanese picks a candidate; it must not run a command.
    if (event.nativeEvent.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      setCursor((current) => (visible.length ? (current + 1) % visible.length : 0));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setCursor((current) => (visible.length ? (current - 1 + visible.length) % visible.length : 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      visible[cursor]?.run();
    }
  };

  return createPortal(
    <div className="fixed inset-0 z-[85] flex items-start justify-center px-4 pt-[16vh]" onKeyDown={onKeyDown}>
      <div
        aria-hidden="true"
        className="animate-backdrop-in absolute inset-0 bg-[hsl(var(--shadow)/0.32)] backdrop-blur-[5px] dark:bg-black/50"
        onMouseDown={onClose}
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("palette.title")}
        data-testid="command-palette"
        className="animate-dialog-in relative flex max-h-[62vh] w-full max-w-[560px] flex-col overflow-hidden rounded-[20px] border border-border bg-card shadow-[0_0_0_1px_hsl(var(--shadow)/0.03),0_32px_90px_-16px_hsl(var(--shadow)/0.45)]"
      >
        <div className="flex items-center gap-3 border-b border-border px-4">
          <IconSearch size={16} className="shrink-0 text-muted-foreground" />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setCursor(0);
            }}
            placeholder={t("palette.placeholder")}
            aria-label={t("palette.title")}
            data-testid="command-palette-input"
            className="h-14 min-w-0 flex-1 bg-transparent text-[15px] text-foreground outline-none placeholder:text-muted-foreground/80"
          />
          <kbd className="shrink-0 rounded-md border border-border bg-muted px-1.5 py-0.5 text-[11px] text-muted-foreground">esc</kbd>
        </div>
        <ul ref={listRef} role="listbox" className="min-h-0 flex-1 overflow-y-auto p-2">
          {visible.length === 0 ? (
            <li className="px-3 py-8 text-center text-[13px] text-muted-foreground">{t("palette.empty")}</li>
          ) : (
            groups.map((group) => {
              const items = visible.filter((command) => command.group === group);
              if (items.length === 0) return null;
              return (
                <li key={group} role="presentation">
                  <p className="px-3 pb-1 pt-2 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
                    {t(`palette.group.${group}`)}
                  </p>
                  <ul role="presentation">
                    {items.map((command) => {
                      const index = visible.indexOf(command);
                      const selected = index === cursor;
                      return (
                        <li
                          key={command.id}
                          role="option"
                          aria-selected={selected}
                          data-testid={`palette-item-${command.id}`}
                          onMouseMove={() => setCursor(index)}
                          onClick={command.run}
                          className={cx(
                            "flex cursor-pointer items-center gap-3 rounded-xl px-3 py-2.5 text-[14px] transition-colors",
                            selected ? "bg-primary/10 text-foreground" : "text-foreground/90",
                          )}
                        >
                          <span
                            className={cx(
                              "flex h-7 w-7 shrink-0 items-center justify-center rounded-lg transition-colors",
                              selected ? "bg-primary/15 text-primary" : "bg-muted text-muted-foreground",
                            )}
                          >
                            {command.icon}
                          </span>
                          <span className="min-w-0 flex-1 truncate">{command.label}</span>
                          {command.hint && selected ? (
                            <span className="shrink-0 text-[12px] text-muted-foreground">{command.hint}</span>
                          ) : null}
                        </li>
                      );
                    })}
                  </ul>
                </li>
              );
            })
          )}
        </ul>
        <div className="flex items-center gap-4 border-t border-border bg-muted/40 px-4 py-2 text-[11.5px] text-muted-foreground">
          <span>↑↓ {t("palette.navigate")}</span>
          <span>↵ {t("palette.run")}</span>
        </div>
      </div>
    </div>,
    document.body,
  );
}
