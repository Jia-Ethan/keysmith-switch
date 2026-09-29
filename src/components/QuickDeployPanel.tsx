import { useEffect, useRef, useState } from "react";
import type { ChangeEvent } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { formatCount } from "../lib/format";
import { trapTab } from "../lib/focus";
import type { ToolId } from "../types";
import { IconClose, IconUpload } from "./icons";
import { MarkdownEditor } from "./MarkdownEditor";
import { ToolLogo } from "./ToolLogos";
import { Button, Field, IconButton, Input, cx } from "./ui";

/** Largest prompt file accepted from import or drop. */
export const MAX_IMPORT_BYTES = 1024 * 1024;
const TEXT_FILE = /\.(md|markdown|mdx|txt)$/i;

export function isImportableFile(file: File): boolean {
  return TEXT_FILE.test(file.name) || file.type.startsWith("text/");
}

/** `Blob.text()` where available, FileReader otherwise (older WebKit, jsdom). */
export function readFileText(file: Blob): Promise<string> {
  if (typeof file.text === "function") return file.text();
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsText(file);
  });
}

export function titleFromFileName(name: string): string {
  return name.replace(/\.[^.]+$/, "").replace(/[-_]+/g, " ").trim();
}

/**
 * The composer for a pasted prompt. It only edits the draft; saving, planning
 * and confirming stay with the workspace, which owns the plan dialog that opens
 * on top of this sheet.
 */
export function QuickDeployPanel({
  open,
  tool,
  toolName,
  title,
  content,
  busy,
  message,
  messageTone,
  canDeploy,
  defaultTitle,
  onTitleChange,
  onContentChange,
  onPreview,
  onSaveOnly,
  onClose,
}: {
  open: boolean;
  tool: ToolId;
  toolName: string;
  title: string;
  content: string;
  busy: boolean;
  message: string | null;
  messageTone: "error" | "muted";
  canDeploy: boolean;
  defaultTitle: string;
  onTitleChange: (value: string) => void;
  onContentChange: (value: string) => void;
  onPreview: () => void;
  onSaveOnly: () => void;
  onClose: () => void;
}) {
  const { t, i18n } = useTranslation();
  const panelRef = useRef<HTMLDivElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const [dropActive, setDropActive] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const latest = useRef({ busy, onClose, title, content, defaultTitle, onTitleChange, onContentChange });
  latest.current = { busy, onClose, title, content, defaultTitle, onTitleChange, onContentChange };

  const importFile = async (file: File) => {
    setImportError(null);
    if (!isImportableFile(file)) {
      setImportError(t("quickDeploy.importUnsupported"));
      return;
    }
    if (file.size > MAX_IMPORT_BYTES) {
      setImportError(t("quickDeploy.importTooLarge"));
      return;
    }
    let text: string;
    try {
      text = await readFileText(file);
    } catch {
      setImportError(t("quickDeploy.importFailed"));
      return;
    }
    const current = latest.current;
    if (current.content.trim() && current.content !== text && !window.confirm(t("quickDeploy.replaceConfirm"))) {
      return;
    }
    current.onContentChange(text);
    if (!current.title.trim() || current.title === current.defaultTitle) {
      const fromName = titleFromFileName(file.name);
      if (fromName) current.onTitleChange(fromName);
    }
  };
  const importRef = useRef(importFile);
  importRef.current = importFile;

  useEffect(() => {
    if (!open) return;
    const panel = panelRef.current;
    if (!panel) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    panel.querySelector<HTMLInputElement>("[data-testid='quick-deploy-title']")?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (trapTab(event, panel)) return;
      if (event.key === "Escape" && !event.defaultPrevented) {
        const target = event.target as HTMLElement | null;
        // Escape inside CodeMirror closes its search panel first.
        if (target?.closest(".cm-panels")) return;
        if (!latest.current.busy) latest.current.onClose();
      }
    };
    const hasFiles = (event: DragEvent) => Array.from(event.dataTransfer?.types ?? []).includes("Files");
    const onDragOver = (event: DragEvent) => {
      if (!hasFiles(event)) return;
      event.preventDefault();
      setDropActive(true);
    };
    const onDragLeave = (event: DragEvent) => {
      if (event.target === panel || !panel.contains(event.relatedTarget as Node | null)) setDropActive(false);
    };
    const onDrop = (event: DragEvent) => {
      if (!hasFiles(event)) return;
      event.preventDefault();
      event.stopPropagation();
      setDropActive(false);
      const file = event.dataTransfer?.files?.[0];
      if (file && !latest.current.busy) void importRef.current(file);
    };
    panel.addEventListener("keydown", onKeyDown, true);
    panel.addEventListener("dragover", onDragOver, true);
    panel.addEventListener("dragleave", onDragLeave, true);
    panel.addEventListener("drop", onDrop, true);
    return () => {
      panel.removeEventListener("keydown", onKeyDown, true);
      panel.removeEventListener("dragover", onDragOver, true);
      panel.removeEventListener("dragleave", onDragLeave, true);
      panel.removeEventListener("drop", onDrop, true);
      previous?.focus();
    };
  }, [open]);

  if (!open || typeof document === "undefined") return null;

  const lines = content ? content.split("\n").length : 0;
  const shownMessage = importError ?? message;
  const tone = importError ? "error" : messageTone;

  return createPortal(
    <div className="titlebar-inset fixed inset-0 z-[60] flex items-stretch justify-center p-3 sm:p-5">
      <div
        className="animate-backdrop-in absolute inset-0 bg-[hsl(var(--shadow)/0.28)] backdrop-blur-[6px] dark:bg-black/50"
        aria-hidden="true"
      />
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={t("quickDeploy.title")}
        tabIndex={-1}
        data-testid="quick-deploy-panel"
        className={cx(
          "animate-panel-in relative flex w-full max-w-[1080px] flex-col overflow-hidden rounded-[22px] border border-border bg-background",
          "shadow-[0_32px_96px_-16px_hsl(var(--shadow)/0.45)]",
          dropActive && "drop-active",
        )}
      >
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-0 top-0 h-40 bg-[radial-gradient(70%_100%_at_10%_0%,rgb(var(--primary)/0.14),transparent_70%)]"
        />
        <header className="relative flex shrink-0 items-center gap-3 px-5 pb-3 pt-4">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-card shadow-pop ring-1 ring-border">
            <ToolLogo tool={tool} size={22} />
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="text-[16px] font-semibold tracking-[-0.01em] text-foreground">{t("quickDeploy.title")}</h2>
            <p className="truncate text-[12.5px] text-muted-foreground">
              {t("quickDeploy.lead", { tool: toolName })}
            </p>
          </div>
          <IconButton label={t("common.close")} size="iconSm" disabled={busy} onClick={onClose} data-testid="quick-deploy-close">
            <IconClose />
          </IconButton>
        </header>

        <div className="relative flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 pb-4">
          <Field label={t("quickDeploy.titleLabel")} hint={t("quickDeploy.titleHint")}>
            <Input
              value={title}
              disabled={busy}
              placeholder={t("quickDeploy.titlePlaceholder")}
              data-testid="quick-deploy-title"
              onChange={(event) => onTitleChange(event.target.value)}
              className="h-10 text-[14px] font-medium"
            />
          </Field>

          <div className="flex min-h-[280px] min-w-0 flex-1 flex-col">
            <div className="mb-1.5 flex items-center gap-2">
              <span className="text-[12.5px] font-medium text-muted-foreground">{t("quickDeploy.contentLabel")}</span>
              <span className="ml-auto" />
              <input
                ref={fileRef}
                type="file"
                accept=".md,.markdown,.mdx,.txt,text/markdown,text/plain"
                className="hidden"
                data-testid="quick-deploy-file"
                onChange={(event: ChangeEvent<HTMLInputElement>) => {
                  const file = event.target.files?.[0];
                  event.target.value = "";
                  if (file) void importFile(file);
                }}
              />
              <Button size="xs" variant="ghost" disabled={busy} data-testid="quick-deploy-import" onClick={() => fileRef.current?.click()}>
                <IconUpload size={13} />
                {t("quickDeploy.import")}
              </Button>
            </div>
            <MarkdownEditor
              value={content}
              onChange={onContentChange}
              placeholder={t("quickDeploy.contentPlaceholder")}
              ariaLabel={t("quickDeploy.contentLabel")}
              readOnly={busy}
              minHeight="100%"
              className="min-h-0 flex-1"
            />
            <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11.5px] text-muted-foreground">
              <span data-testid="quick-deploy-stats" className="tabular-nums">
                {t("quickDeploy.stats", {
                  chars: formatCount(content.length, i18n.language),
                  lines: formatCount(lines, i18n.language),
                })}
              </span>
              <span className="ml-auto">{t("quickDeploy.dropHint")}</span>
              <span>{t("editor.findHint")}</span>
            </div>
          </div>
        </div>

        <footer className="relative flex shrink-0 flex-wrap items-center gap-2 border-t border-border bg-card/70 px-5 py-3">
          <p
            data-testid="quick-deploy-status"
            role={tone === "error" ? "alert" : undefined}
            className={cx("min-w-0 flex-1 truncate text-[12.5px]", tone === "error" ? "text-destructive" : "text-muted-foreground")}
          >
            {shownMessage ?? ""}
          </p>
          <Button variant="outline" disabled={busy} data-testid="quick-deploy-save" onClick={onSaveOnly}>
            {t("quickDeploy.saveOnly")}
          </Button>
          <Button
            variant="primary"
            loading={busy}
            disabled={!canDeploy}
            data-testid="quick-deploy-submit"
            onClick={onPreview}
          >
            {busy ? t("common.busy") : t("quickDeploy.preview")}
          </Button>
        </footer>
      </div>
    </div>,
    document.body,
  );
}
