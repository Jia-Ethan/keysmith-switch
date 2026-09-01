import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ConfirmDialog } from "./ConfirmDialog";
import { ErrorBanner } from "./ErrorBanner";
import { FullScreenPanel } from "./FullScreenPanel";
import { Button, Field, Textarea, Disclosure, SectionLabel, cx } from "./ui";
import { IconPencil, IconPower, IconCopy, IconMore, IconTrash } from "./icons";
import type { ToastApi } from "../hooks/useToasts";
import type { PromptDetail, PromptVersion, ToolId } from "../types";
import { TOOL_IDS } from "../types";

export function PromptViewPage({
  promptId,
  tool,
  isActiveHere,
  disabled,
  busy,
  toast,
  onClose,
  onLoaded,
  onEdit,
  onActivate,
  onDeactivate,
  onChanged,
  onOpenPrompt,
  onDeleted,
}: {
  promptId: string;
  tool: ToolId;
  /** null when activation state could not be read */
  isActiveHere: boolean | null;
  disabled: boolean;
  busy: boolean;
  toast: ToastApi;
  onClose: () => void;
  onLoaded?: (detail: PromptDetail) => void;
  onEdit: (detail: PromptDetail) => void;
  onActivate: () => void;
  onDeactivate: () => void;
  onChanged?: () => void;
  onOpenPrompt?: (id: string) => void;
  onDeleted: () => void;
}) {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<PromptDetail | null>(null);
  const [versions, setVersions] = useState<PromptVersion[]>([]);
  const [historyError, setHistoryError] = useState(false);
  const [diff, setDiff] = useState("");
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [loadEpoch, setLoadEpoch] = useState(0);
  const [menuOpen, setMenuOpen] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const menuId = useId();
  const menuRef = useRef<HTMLDivElement>(null);
  const menuTriggerRef = useRef<HTMLButtonElement>(null);
  const loadedRef = useRef(onLoaded);
  const toastRef = useRef(toast);
  loadedRef.current = onLoaded;
  toastRef.current = toast;
  const locked = busy || actionBusy;

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      setLoading(true);
      setLoadError(false);
      try {
        const promptData = await api.getPrompt(promptId);
        if (cancelled) return;
        setDetail(promptData);
        loadedRef.current?.(promptData);
        try {
          const historyData = await api.promptHistory(promptId);
          if (cancelled) return;
          setVersions(historyData.versions ?? []);
          setHistoryError(false);
        } catch {
          if (cancelled) return;
          setVersions([]);
          setHistoryError(true);
        }
      } catch (err) {
        if (cancelled) return;
        toastRef.current.err(err);
        setLoadError(true);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, [loadEpoch, promptId]);

  useEffect(() => {
    if (!menuOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        setMenuOpen(false);
        menuTriggerRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [menuOpen]);

  useEffect(() => {
    if (!menuOpen) return;
    menuRef.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
  }, [menuOpen]);

  const runAction = async (action: () => Promise<void>) => {
    if (locked) return;
    setActionBusy(true);
    try {
      await action();
    } catch (err) {
      toast.err(err);
    } finally {
      setActionBusy(false);
    }
  };

  const duplicateSameTool = async () => {
    if (!detail) return;
    await runAction(async () => {
      const created = await api.createPrompt({
        tool,
        title: `${detail.title} copy`,
        content: detail.content,
        tags: detail.tags,
      });
      toast.ok(t("prompts.created"));
      onChanged?.();
      if (onOpenPrompt) onOpenPrompt(created.id);
      else onClose();
    });
  };

  const copyTo = async (targetTool: ToolId) => {
    if (!detail) return;
    await runAction(async () => {
      await api.copyPrompt(detail.id, targetTool);
      toast.ok(t("prompts.copied"));
      setMenuOpen(false);
    });
  };

  const removePrompt = async () => {
    if (!detail) return;
    await runAction(async () => {
      await api.deletePrompt(detail.id);
      toast.ok(t("prompts.deleted"));
      setDeleteOpen(false);
      onChanged?.();
      onDeleted();
    });
  };

  const restoreVersion = async (version: number) => {
    if (!detail) return;
    await runAction(async () => {
      const restored = await api.restorePromptVersion(detail.id, version);
      toast.ok(t("prompts.restored"));
      setDetail(restored);
      loadedRef.current?.(restored);
      try {
        const history = await api.promptHistory(detail.id);
        setVersions(history.versions ?? []);
        setHistoryError(false);
      } catch (err) {
        setHistoryError(true);
        toast.err(err);
      }
      onChanged?.();
    });
  };

  const showDiff = async (fromVersion: number, toVersion: number) => {
    if (!detail) return;
    await runAction(async () => {
      const result = await api.promptDiff(detail.id, fromVersion, toVersion);
      setDiff(result.unified || result.summary || "");
    });
  };

  const retryHistory = async () => {
    await runAction(async () => {
      const history = await api.promptHistory(promptId);
      setVersions(history.versions ?? []);
      setHistoryError(false);
    });
  };

  if (loading) {
    return (
      <FullScreenPanel isOpen title={t("common.loading")} onClose={onClose}>
        <div className="flex items-center justify-center py-12">
          <p className="text-muted-foreground">{t("common.busy")}</p>
        </div>
      </FullScreenPanel>
    );
  }

  if (loadError || !detail) {
    return (
      <FullScreenPanel isOpen title={t("errors.loadFailed")} onClose={onClose}>
        <div className="mx-auto w-full max-w-3xl">
          <ErrorBanner
            message={t("errors.loadFailed")}
            retryLabel={t("common.retry")}
            onRetry={() => setLoadEpoch((value) => value + 1)}
          />
        </div>
      </FullScreenPanel>
    );
  }

  return (
    <FullScreenPanel
      isOpen
      title={detail.title}
      onClose={onClose}
      closeDisabled={locked}
      footer={
        <div className="flex w-full flex-wrap items-center justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2">
            {isActiveHere === null ? (
              <span
                className="inline-flex items-center rounded-xl border border-amber-600/30 bg-amber-500/10 px-2.5 py-1 text-[14px] text-amber-700 dark:text-amber-300"
                role="status"
                data-testid="prompt-activation-unknown"
              >
                {t("prompts.activationUnknownShort")}
              </span>
            ) : null}
            {isActiveHere === true ? (
              <span
                className="inline-flex shrink-0 items-center rounded-lg border border-primary/30 bg-primary/10 px-2 py-1 text-[13px] font-medium text-primary"
                data-testid="prompt-active-here"
              >
                {t("status.active")}
              </span>
            ) : null}
          </div>
          <div className="ml-auto flex flex-wrap items-center justify-end gap-2">
            {isActiveHere === true ? (
              <Button
                size="sm"
                disabled={disabled || locked}
                data-testid="prompt-deactivate"
                onClick={onDeactivate}
              >
                <IconPower />
                {t("prompts.deactivate")}
              </Button>
            ) : null}
            {isActiveHere === false ? (
              <Button
                size="sm"
                variant="primary"
                disabled={disabled || locked}
                data-testid="prompt-activate"
                onClick={onActivate}
              >
                <IconPower />
                {t("prompts.activate")}
              </Button>
            ) : null}
            <Button
              size="sm"
              disabled={locked}
              data-testid="prompt-edit"
              onClick={() => onEdit(detail)}
            >
              <IconPencil />
              {t("prompts.edit")}
            </Button>
            <div className="relative">
              <Button
                ref={menuTriggerRef}
                size="sm"
                variant="ghost"
                disabled={locked}
                data-testid="prompt-menu"
                title={t("common.details")}
                aria-label={t("common.details")}
                aria-haspopup="menu"
                aria-expanded={menuOpen}
                aria-controls={menuOpen ? menuId : undefined}
                onClick={() => setMenuOpen(!menuOpen)}
              >
                <IconMore />
              </Button>
              {menuOpen ? (
                <>
                  <div
                    className="fixed inset-0 z-20"
                    onClick={() => setMenuOpen(false)}
                    aria-hidden="true"
                  />
                  <div
                    id={menuId}
                    ref={menuRef}
                    role="menu"
                    className="absolute bottom-full right-0 mb-2 w-52 overflow-hidden rounded-xl border border-border bg-card py-1 shadow-lg"
                    style={{ zIndex: 30 }}
                    onKeyDown={(event) => {
                      if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
                      event.preventDefault();
                      const items = Array.from(
                        menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [],
                      );
                      if (!items.length) return;
                      const current = items.indexOf(document.activeElement as HTMLElement);
                      const next =
                        event.key === "Home"
                          ? 0
                          : event.key === "End"
                            ? items.length - 1
                            : event.key === "ArrowDown"
                              ? (current + 1 + items.length) % items.length
                              : (current - 1 + items.length) % items.length;
                      items[next]?.focus();
                    }}
                  >
                    <MenuItem
                      testId="prompt-duplicate"
                      onSelect={() => {
                        setMenuOpen(false);
                        void duplicateSameTool();
                      }}
                    >
                      <IconCopy />
                      {t("prompts.copy")}
                    </MenuItem>
                    <div className="my-1 border-t border-border" />
                    <p className="px-3 py-1.5 text-[13px] font-semibold uppercase tracking-wide text-muted-foreground">
                      {t("prompts.copyTo")}
                    </p>
                    {TOOL_IDS.filter((item) => item !== tool).map((target) => (
                      <MenuItem
                        key={target}
                        testId={`prompt-copy-${target}`}
                        onSelect={() => void copyTo(target)}
                      >
                        <span className="w-3.5" aria-hidden="true" />
                        {t(`nav.${target}`)}
                      </MenuItem>
                    ))}
                    <div className="my-1 border-t border-border" />
                    <MenuItem
                      testId="prompt-delete"
                      danger
                      onSelect={() => {
                        setMenuOpen(false);
                        setDeleteOpen(true);
                      }}
                    >
                      <IconTrash />
                      {t("prompts.delete")}
                    </MenuItem>
                  </div>
                </>
              ) : null}
            </div>
          </div>
        </div>
      }
    >
      <div className="mx-auto w-full max-w-5xl space-y-5">
        <div className="grid gap-4 sm:grid-cols-2">
          <Field label={t("prompts.title")}>
            <div className="rounded-xl border border-border bg-muted/35 px-3 py-2 text-[15px]">
              {detail.title}
            </div>
          </Field>
          <Field label={t("prompts.tags")}>
            <div className="rounded-xl border border-border bg-muted/35 px-3 py-2 text-[15px]">
              {detail.tags.join(", ") || "—"}
            </div>
          </Field>
        </div>

        <Field label={t("prompts.content")}>
          <Textarea
            value={detail.content}
            rows={18}
            readOnly
            className={cx("min-h-[420px] bg-muted/35")}
          />
        </Field>

        {historyError ? (
          <ErrorBanner
            message={t("history.loadFailed")}
            retryLabel={t("common.retry")}
            onRetry={() => void retryHistory()}
          />
        ) : null}

        {versions.length > 0 ? (
          <Disclosure
            title={`${t("history.title")} (${versions.length})`}
            testId="prompt-history"
          >
            <ul className="flex flex-col gap-1">
              {versions.map((item, index) => (
                <li
                  key={item.version}
                  className="flex flex-wrap items-center gap-2 rounded-lg border border-border px-2.5 py-2 text-[13px]"
                >
                  <span className="font-mono font-medium text-foreground">
                    {t("history.version")} {item.version}
                  </span>
                  <span className="text-muted-foreground">{item.createdAt}</span>
                  <div className="ml-auto flex shrink-0 gap-1">
                    {index < versions.length - 1 ? (
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={locked}
                        onClick={() =>
                          showDiff(item.version, versions[index + 1]!.version)
                        }
                      >
                        {t("history.diff")}
                      </Button>
                    ) : null}
                    <Button
                      size="sm"
                      disabled={locked}
                      onClick={() => restoreVersion(item.version)}
                    >
                      {t("prompts.restore")}
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
            {diff ? (
              <div className="mt-2">
                <SectionLabel>{t("history.diff")}</SectionLabel>
                <pre className="mt-1 max-h-64 overflow-auto whitespace-pre-wrap rounded-lg border border-border bg-muted/50 px-2.5 py-2 font-mono text-[13px] leading-snug">
                  {diff}
                </pre>
              </div>
            ) : null}
          </Disclosure>
        ) : null}
      </div>
      <ConfirmDialog
        open={deleteOpen}
        title={t("prompts.delete")}
        description={t("prompts.deleteConfirm")}
        confirmLabel={actionBusy ? t("common.busy") : t("prompts.delete")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        danger
        busy={actionBusy}
        confirmDisabled={locked}
        confirmTestId="prompt-delete-confirm"
        onClose={() => {
          if (!actionBusy) setDeleteOpen(false);
        }}
        onConfirm={() => void removePrompt()}
      >
        <p className="text-sm font-medium text-foreground">{detail.title}</p>
      </ConfirmDialog>
    </FullScreenPanel>
  );
}

function MenuItem({
  children,
  onSelect,
  danger,
  testId,
}: {
  children: React.ReactNode;
  onSelect: () => void;
  danger?: boolean;
  testId?: string;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      data-testid={testId}
      onClick={onSelect}
      className={cx(
        "flex w-full items-center gap-2 px-3 py-2 text-left text-[15px] transition-colors",
        "focus-visible:bg-muted focus-visible:outline-none",
        danger
          ? "text-destructive hover:bg-destructive/10"
          : "text-foreground hover:bg-muted",
      )}
    >
      {children}
    </button>
  );
}
