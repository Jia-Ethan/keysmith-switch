import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { PromptSummary } from "../types";
import { EmptyState } from "./EmptyState";
import { cx, Tag } from "./ui";

export interface PromptListProps {
  prompts: PromptSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  /**
   * ids active for the current tool + scope + project.
   * `null` means the activation table could not be read, so Active / Inactive
   * is genuinely unknown and must not be guessed.
   * `undefined` falls back to the per-row flag.
   */
  activeIds?: string[] | null;
  loading?: boolean;
  /** true when a search query or tag filter is applied */
  filtered?: boolean;
  emptyAction?: ReactNode;
}

export function PromptList({
  prompts,
  selectedId,
  onSelect,
  activeIds,
  loading = false,
  filtered = false,
  emptyAction,
}: PromptListProps) {
  const { t } = useTranslation();

  if (loading) {
    return (
      <div
        className="flex min-h-[148px] items-center justify-center px-4 py-8 text-[14px] text-muted-foreground"
        data-testid="prompt-list-loading"
        role="status"
      >
        <span className="mr-2 h-3.5 w-3.5 animate-spin rounded-full border-2 border-primary/25 border-t-primary" aria-hidden="true" />
        <span>{t("common.loading")}</span>
      </div>
    );
  }

  if (prompts.length === 0) {
    return filtered ? (
      <EmptyState
        title={t("prompts.noResults")}
        hint={t("prompts.noResultsHint")}
        testId="prompt-list-no-results"
      />
    ) : (
      <EmptyState
        title={t("prompts.empty")}
        hint={t("prompts.emptyHint")}
        action={emptyAction}
        testId="prompt-list-empty"
      />
    );
  }

  // Activation state unknown: show one flat list rather than filing every prompt
  // under "Inactive", which would read as "nothing is applied".
  if (activeIds === null) {
    return (
      <div className="flex flex-col gap-3" data-testid="prompt-list">
        <p
          className="rounded-2xl border border-amber-600/40 bg-amber-500/10 px-3 py-2 text-[14px] leading-snug text-amber-700 dark:text-amber-400"
          role="status"
          data-testid="prompt-activation-unknown"
        >
          {t("prompts.activationUnknown")}
        </p>
        <Group
          title={t("prompts.allPrompts")}
          items={prompts}
          selectedId={selectedId}
          onSelect={onSelect}
        />
      </div>
    );
  }

  const isActive = (item: PromptSummary) =>
    activeIds ? activeIds.includes(item.id) : item.active;
  const active = prompts.filter(isActive);
  const inactive = prompts.filter((item) => !isActive(item));

  return (
    <div className="flex min-h-0 flex-col gap-3" data-testid="prompt-list">
      <Group
        title={t("prompts.active")}
        items={active}
        selectedId={selectedId}
        onSelect={onSelect}
        activeGroup
      />
      <Group
        title={t("prompts.inactive")}
        items={inactive}
        selectedId={selectedId}
        onSelect={onSelect}
      />
    </div>
  );
}

function Group({
  title,
  items,
  selectedId,
  onSelect,
  activeGroup = false,
}: {
  title: string;
  items: PromptSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  activeGroup?: boolean;
}) {
  const { t } = useTranslation();
  return (
    <section
      className={cx(
        "flex min-w-0 flex-col overflow-hidden rounded-2xl border bg-card",
        activeGroup ? "border-primary/35 shadow-[0_10px_30px_hsl(var(--primary)/0.08)]" : "border-border",
      )}
    >
      <h3 className="mb-0 flex items-center gap-1 border-b border-border px-4 py-2.5 text-[14px] font-semibold text-foreground">
        {title}
        <span className="font-normal tabular-nums">({items.length})</span>
      </h3>
      {items.length === 0 ? (
        <p className="px-4 py-4 text-sm text-muted-foreground">
          {activeGroup ? t("prompts.noneActive") : t("prompts.noneInactive")}
        </p>
      ) : (
        <ul className="grid grid-cols-1 gap-2 p-3 sm:grid-cols-2 xl:grid-cols-3">
          {items.map((item) => {
            const selected = selectedId === item.id;
            return (
              <li key={item.id}>
                <button
                  type="button"
                  onClick={() => onSelect(item.id)}
                  aria-current={selected ? "true" : undefined}
                  data-testid={`prompt-item-${item.id}`}
                  className={cx(
                    "h-full w-full rounded-xl border px-3 py-3 text-left transition-colors",
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    selected
                      ? "border-primary/50 bg-primary/10 shadow-sm"
                      : "border-border/70 bg-background/60 hover:border-primary/25 hover:bg-muted/70",
                  )}
                >
                  <div className="flex items-center gap-1.5">
                    {activeGroup ? (
                      <span
                        className="h-1.5 w-1.5 shrink-0 rounded-full bg-primary"
                        aria-hidden="true"
                      />
                    ) : null}
                    <span
                      className={cx(
                        "min-w-0 flex-1 truncate text-[17px]",
                        selected ? "font-semibold text-foreground" : "font-medium text-foreground",
                      )}
                    >
                      {item.title}
                    </span>
                  </div>
                  {item.tags.length > 0 || item.lastUsedAt || item.updatedAt ? (
                    <div className="mt-1 flex min-w-0 items-center gap-1 overflow-hidden">
                      {item.tags.slice(0, 2).map((tag) => (
                        <Tag key={tag}>{tag}</Tag>
                      ))}
                      {item.tags.length > 2 ? (
                        <span className="shrink-0 text-[13px] text-muted-foreground">
                          +{item.tags.length - 2}
                        </span>
                      ) : null}
                      <span className="ml-auto shrink-0 text-[13px] tabular-nums text-muted-foreground">
                        {shortDate(item.lastUsedAt ?? item.updatedAt)}
                      </span>
                    </div>
                  ) : null}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function shortDate(value: string | null): string {
  if (!value) return "";
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value.slice(0, 10);
  const now = Date.now();
  const diffDays = Math.floor((now - parsed.getTime()) / 86_400_000);
  if (diffDays <= 0) return parsed.toISOString().slice(11, 16);
  return parsed.toISOString().slice(0, 10);
}
