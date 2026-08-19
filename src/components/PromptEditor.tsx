import { useState } from "react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { PromptDetail, PromptVersion, ToolId } from "../types";
import { TOOL_IDS } from "../types";
import { Button, Disclosure, Field, IconButton, Input, SectionLabel, Textarea, cx } from "./ui";
import { IconCopy, IconMore, IconPencil, IconPower, IconTrash } from "./icons";

export interface PromptDraft {
  title: string;
  content: string;
  tags: string;
}

/**
 * Read / edit pane for one prompt. Primary actions stay visible; secondary
 * actions (duplicate, cross-tool copy, delete) live in an overflow menu so the
 * header never turns into a single long button row.
 */
export function PromptEditor({
  tool,
  detail,
  draft,
  creating,
  editing,
  busy,
  disabled,
  isActiveHere,
  versions,
  diff,
  onDraftChange,
  onStartEdit,
  onCancelEdit,
  onSave,
  onDuplicate,
  onCopyTo,
  onDelete,
  onActivate,
  onDeactivate,
  onRestoreVersion,
  onShowDiff,
}: {
  tool: ToolId;
  detail: PromptDetail | null;
  draft: PromptDraft;
  creating: boolean;
  editing: boolean;
  busy: boolean;
  disabled: boolean;
  /** null when activation state could not be read; do not claim either state. */
  isActiveHere: boolean | null;
  versions: PromptVersion[];
  diff: string;
  onDraftChange: (draft: PromptDraft) => void;
  onStartEdit: () => void;
  onCancelEdit: () => void;
  onSave: () => void;
  onDuplicate: () => void;
  onCopyTo: (target: ToolId) => void;
  onDelete: () => void;
  onActivate: () => void;
  onDeactivate: () => void;
  onRestoreVersion: (version: number) => void;
  onShowDiff: (from: number, to: number) => void;
}) {
  const { t } = useTranslation();
  const [menuOpen, setMenuOpen] = useState(false);
  const readOnly = !creating && !editing;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-1.5">
        <div className="flex min-w-0 flex-1 items-center gap-2">
          <h2 className="min-w-0 truncate text-[12px] font-semibold text-foreground">
            {creating ? t("prompts.new") : detail?.title || "—"}
          </h2>
          {!creating && isActiveHere === true ? (
            <span
              className="inline-flex shrink-0 items-center rounded border border-primary/30 bg-primary/10 px-1.5 py-px text-[10px] font-medium text-primary"
              data-testid="prompt-active-here"
            >
              {t("status.active")}
            </span>
          ) : null}
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {creating || editing ? (
            <>
              <Button size="sm" disabled={busy} onClick={onCancelEdit}>
                {t("common.cancel")}
              </Button>
              <Button
                size="sm"
                variant="primary"
                disabled={busy}
                data-testid="prompt-save"
                onClick={onSave}
              >
                {t("prompts.save")}
              </Button>
            </>
          ) : detail ? (
            <>
              {isActiveHere !== false ? (
                <Button
                  size="sm"
                  disabled={disabled || busy}
                  data-testid="prompt-deactivate"
                  onClick={onDeactivate}
                >
                  <IconPower />
                  {t("prompts.deactivate")}
                </Button>
              ) : null}
              {isActiveHere !== true ? (
                <Button
                  size="sm"
                  variant="primary"
                  disabled={disabled || busy}
                  data-testid="prompt-activate"
                  onClick={onActivate}
                >
                  <IconPower />
                  {t("prompts.activate")}
                </Button>
              ) : null}
              <IconButton label={t("prompts.edit")} data-testid="prompt-edit" onClick={onStartEdit}>
                <IconPencil />
              </IconButton>
              <PromptMenu
                tool={tool}
                open={menuOpen}
                busy={busy}
                onOpenChange={setMenuOpen}
                onDuplicate={onDuplicate}
                onCopyTo={onCopyTo}
                onDelete={onDelete}
              />
            </>
          ) : null}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto px-3 py-3">
        <div className="flex flex-col gap-3">
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label={t("prompts.title")}>
              <Input
                value={draft.title}
                disabled={busy}
                readOnly={readOnly}
                data-testid="prompt-title"
                onChange={(event) => onDraftChange({ ...draft, title: event.target.value })}
              />
            </Field>
            <Field label={t("prompts.tags")} hint={readOnly ? undefined : t("prompts.tagsPlaceholder")}>
              <Input
                value={draft.tags}
                disabled={busy}
                readOnly={readOnly}
                placeholder={readOnly ? undefined : t("prompts.tagsPlaceholder")}
                data-testid="prompt-tags"
                onChange={(event) => onDraftChange({ ...draft, tags: event.target.value })}
              />
            </Field>
          </div>

          <Field label={t("prompts.content")}>
            <Textarea
              value={draft.content}
              rows={16}
              disabled={busy}
              readOnly={readOnly}
              data-testid="prompt-content"
              onChange={(event) => onDraftChange({ ...draft, content: event.target.value })}
              className={cx("min-h-[240px] max-h-[52vh]", readOnly && "bg-muted/40")}
            />
          </Field>

          {!creating && versions.length > 0 ? (
            <Disclosure title={`${t("history.title")} (${versions.length})`} testId="prompt-history">
              <ul className="flex flex-col gap-1">
                {versions.map((item, index) => (
                  <li
                    key={item.version}
                    className="flex flex-wrap items-center gap-2 rounded border border-border px-2 py-1.5 text-[11px]"
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
                          disabled={busy}
                          onClick={() => onShowDiff(item.version, versions[index + 1]!.version)}
                        >
                          {t("history.diff")}
                        </Button>
                      ) : null}
                      <Button
                        size="sm"
                        disabled={busy}
                        onClick={() => onRestoreVersion(item.version)}
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
                  <pre className="mt-1 max-h-64 overflow-auto whitespace-pre-wrap rounded border border-border bg-muted/50 px-2 py-1.5 font-mono text-[11px] leading-relaxed">
                    {diff}
                  </pre>
                </div>
              ) : null}
            </Disclosure>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function PromptMenu({
  tool,
  open,
  busy,
  onOpenChange,
  onDuplicate,
  onCopyTo,
  onDelete,
}: {
  tool: ToolId;
  open: boolean;
  busy: boolean;
  onOpenChange: (open: boolean) => void;
  onDuplicate: () => void;
  onCopyTo: (target: ToolId) => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const targets = TOOL_IDS.filter((item) => item !== tool);

  return (
    <div className="relative">
      <IconButton
        label={t("common.details")}
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={busy}
        data-testid="prompt-menu"
        onClick={() => onOpenChange(!open)}
      >
        <IconMore />
      </IconButton>
      {open ? (
        <>
          <div className="fixed inset-0 z-20" onClick={() => onOpenChange(false)} aria-hidden="true" />
          <div
            role="menu"
            className="absolute right-0 top-8 z-30 w-52 overflow-hidden rounded-md border border-border bg-card py-1 shadow-lg"
          >
            <MenuItem
              testId="prompt-duplicate"
              onSelect={() => {
                onOpenChange(false);
                onDuplicate();
              }}
            >
              <IconCopy />
              {t("prompts.copy")}
            </MenuItem>

            <div className="my-1 border-t border-border" />
            <p className="px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              {t("prompts.copyTo")}
            </p>
            {targets.map((target) => (
              <MenuItem
                key={target}
                testId={`prompt-copy-${target}`}
                onSelect={() => {
                  onOpenChange(false);
                  onCopyTo(target);
                }}
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
                onOpenChange(false);
                onDelete();
              }}
            >
              <IconTrash />
              {t("prompts.delete")}
            </MenuItem>
          </div>
        </>
      ) : null}
    </div>
  );
}

function MenuItem({
  children,
  onSelect,
  danger,
  testId,
}: {
  children: ReactNode;
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
        "flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-[12px] transition-colors",
        "focus-visible:outline-none focus-visible:bg-muted",
        danger ? "text-destructive hover:bg-destructive/10" : "text-foreground hover:bg-muted",
      )}
    >
      {children}
    </button>
  );
}
