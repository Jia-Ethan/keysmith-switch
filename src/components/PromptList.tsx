import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { relativeTime } from "../lib/format";
import type { PromptSummary } from "../types";
import { EmptyState } from "./EmptyState";
import { IconChevronRight, IconLibrary, IconRocket } from "./icons";
import { Button, cx, Tag } from "./ui";

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
  /**
   * The machine runs a prompt the library has no entry for. It gets a card of
   * its own in the live group, so the list never contradicts the hero.
   */
  unrecordedLive?: boolean;
  emptyAction?: ReactNode;
  /** One-click deploy from a card. Omitted: cards only open the detail page. */
  onDeploy?: (id: string) => void;
  deployDisabled?: boolean;
  /** The card whose deploy plan is being prepared; its button shows progress. */
  deployingId?: string | null;
}

export function PromptList({
  prompts,
  selectedId,
  onSelect,
  activeIds,
  loading = false,
  filtered = false,
  unrecordedLive = false,
  emptyAction,
  onDeploy,
  deployDisabled = false,
  deployingId = null,
}: PromptListProps) {
  const { t } = useTranslation();

  if (loading) {
    return (
      <div data-testid="prompt-list-loading" role="status" aria-label={t("common.loading")}>
        <ul className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3" aria-hidden="true">
          {Array.from({ length: 6 }, (_, index) => (
            <li key={index} className="surface-card flex h-[132px] flex-col gap-2.5 p-4">
              <div className="skeleton h-4 w-2/3" />
              <div className="skeleton h-3 w-full" />
              <div className="skeleton h-3 w-4/5" />
              <div className="mt-auto flex gap-1.5">
                <div className="skeleton h-5 w-12" />
                <div className="skeleton h-5 w-10" />
              </div>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  const showLive = unrecordedLive && !filtered;

  if (prompts.length === 0 && showLive) {
    return (
      <div className="flex flex-col gap-4" data-testid="prompt-list">
        <Group title={t("prompts.active")} items={[]} selectedId={selectedId} onSelect={onSelect} activeGroup unrecordedLive />
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
        icon={<IconLibrary size={22} />}
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
      <div className="flex flex-col gap-4" data-testid="prompt-list">
        <p
          className="rounded-xl border border-warning/35 bg-warning/10 px-3 py-2 text-[12.5px] leading-snug text-warning"
          role="status"
          data-testid="prompt-activation-unknown"
        >
          {t("prompts.activationUnknown")}
        </p>
        {showLive ? (
          <Group title={t("prompts.active")} items={[]} selectedId={selectedId} onSelect={onSelect} activeGroup unrecordedLive />
        ) : null}
        <Group title={t("prompts.allPrompts")} items={prompts} selectedId={selectedId} onSelect={onSelect} />
      </div>
    );
  }

  const isActive = (item: PromptSummary) => (activeIds ? activeIds.includes(item.id) : item.active);
  const active = prompts.filter(isActive);
  const inactive = prompts.filter((item) => !isActive(item));

  return (
    <div className="flex min-h-0 flex-col gap-6" data-testid="prompt-list">
      {active.length > 0 || showLive ? (
        <Group
          title={t("prompts.active")}
          items={active}
          selectedId={selectedId}
          onSelect={onSelect}
          activeGroup
          unrecordedLive={showLive}
        />
      ) : null}
      <Group
        title={t("prompts.inactive")}
        items={inactive}
        selectedId={selectedId}
        onSelect={onSelect}
        onDeploy={onDeploy}
        deployDisabled={deployDisabled}
        deployingId={deployingId}
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
  unrecordedLive = false,
  onDeploy,
  deployDisabled = false,
  deployingId = null,
}: {
  title: string;
  items: PromptSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  activeGroup?: boolean;
  unrecordedLive?: boolean;
  onDeploy?: (id: string) => void;
  deployDisabled?: boolean;
  deployingId?: string | null;
}) {
  const { t, i18n } = useTranslation();
  return (
    <section className="flex min-w-0 flex-col gap-2.5" data-group={activeGroup ? "active" : "library"}>
      <h3 className="flex items-center gap-2 px-0.5 text-[12px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
        {activeGroup ? <span className="status-dot text-primary" data-live="" aria-hidden="true" /> : null}
        {title}
        <span className="rounded-full bg-muted px-1.5 py-px text-[11px] font-medium tabular-nums tracking-normal">
          {items.length + (unrecordedLive ? 1 : 0)}
        </span>
      </h3>
      {items.length === 0 && !unrecordedLive ? (
        <p className="rounded-xl border border-dashed border-border px-4 py-5 text-center text-[12.5px] text-muted-foreground">
          {activeGroup ? t("prompts.noneActive") : t("prompts.noneInactive")}
        </p>
      ) : (
        <ul className={cx("grid grid-cols-1 gap-3", activeGroup ? "" : "sm:grid-cols-2 xl:grid-cols-3")}>
          {unrecordedLive ? <UnrecordedLiveCard /> : null}
          {items.map((item, index) => {
            const selected = selectedId === item.id;
            return (
              <li
                key={item.id}
                className="animate-rise group relative"
                style={{ animationDelay: `${Math.min(index, 8) * 35}ms` }}
              >
                <button
                  type="button"
                  onClick={() => onSelect(item.id)}
                  aria-current={selected ? "true" : undefined}
                  data-testid={`prompt-item-${item.id}`}
                  className={cx(
                    "flex h-full w-full flex-col rounded-2xl border p-4 text-left",
                    "transition-[border-color,box-shadow,transform,background-color] duration-200",
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    activeGroup
                      ? "border-primary/40 bg-[linear-gradient(135deg,rgb(var(--primary)/0.10),rgb(var(--primary)/0.02)_55%,transparent)] bg-card shadow-glow"
                      : cx(
                          "surface-card hover:-translate-y-0.5 hover:border-primary/30",
                          "hover:shadow-[0_1px_2px_hsl(var(--shadow)/0.05),0_16px_32px_-16px_rgb(var(--primary)/0.35)]",
                        ),
                    selected && "ring-2 ring-primary/40",
                  )}
                >
                  <div className="flex w-full items-start gap-2">
                    <span
                      className={cx(
                        "min-w-0 flex-1 truncate font-semibold tracking-[-0.01em] text-foreground",
                        activeGroup ? "text-[16px]" : "text-[14px]",
                      )}
                    >
                      {item.title}
                    </span>
                    {activeGroup ? (
                      <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-primary px-2 py-0.5 text-[11px] font-semibold text-primary-foreground">
                        {t("status.live")}
                      </span>
                    ) : (
                      <span className="shrink-0 pt-0.5 text-[11.5px] tabular-nums text-muted-foreground">
                        {relativeTime(item.lastUsedAt ?? item.updatedAt, i18n.language)}
                      </span>
                    )}
                  </div>
                  <p
                    className={cx(
                      "mt-1.5 text-[12.5px] leading-relaxed text-muted-foreground",
                      activeGroup ? "line-clamp-3" : "line-clamp-2 min-h-[2.6em]",
                    )}
                  >
                    {item.excerpt || t("prompts.noExcerpt")}
                  </p>
                  <div className={cx("mt-3 flex min-w-0 items-center gap-1", onDeploy && !activeGroup && "pr-20")}>
                    {item.tags.slice(0, 3).map((tag) => (
                      <Tag key={tag}>{tag}</Tag>
                    ))}
                    {item.tags.length > 3 ? (
                      <span className="shrink-0 text-[11.5px] text-muted-foreground">+{item.tags.length - 3}</span>
                    ) : null}
                    {activeGroup ? (
                      <span className="ml-auto inline-flex shrink-0 items-center gap-0.5 text-[12px] font-medium text-primary">
                        {relativeTime(item.lastUsedAt ?? item.updatedAt, i18n.language)}
                        <IconChevronRight size={13} />
                      </span>
                    ) : null}
                  </div>
                </button>
                {onDeploy && !activeGroup ? (
                  <Button
                    size="xs"
                    variant="subtle"
                    loading={deployingId === item.id}
                    disabled={deployDisabled && deployingId !== item.id}
                    data-testid={`prompt-deploy-${item.id}`}
                    title={t("prompts.deployThis")}
                    onClick={() => onDeploy(item.id)}
                    className={cx(
                      "absolute bottom-3.5 right-3.5 transition-opacity",
                      deployingId === item.id
                        ? "opacity-100"
                        : "opacity-100 sm:opacity-0 sm:focus-visible:opacity-100 sm:group-hover:opacity-100",
                    )}
                  >
                    <IconRocket size={13} />
                    {t("prompts.deploy")}
                  </Button>
                ) : null}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

/** The live prompt on the machine has no library entry, so nothing here opens or deploys. */
function UnrecordedLiveCard() {
  const { t } = useTranslation();
  return (
    <li className="animate-rise" data-testid="prompt-live-unrecorded">
      <div className="flex h-full w-full flex-col rounded-2xl border border-primary/40 bg-[linear-gradient(135deg,rgb(var(--primary)/0.10),rgb(var(--primary)/0.02)_55%,transparent)] bg-card p-4 shadow-glow">
        <div className="flex w-full items-start gap-2">
          <span className="min-w-0 flex-1 truncate text-[16px] font-semibold tracking-[-0.01em] text-foreground">
            {t("prompts.liveUnrecorded")}
          </span>
          <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-primary px-2 py-0.5 text-[11px] font-semibold text-primary-foreground">
            {t("status.live")}
          </span>
        </div>
        <p className="mt-1.5 text-[12.5px] leading-relaxed text-muted-foreground">{t("prompts.liveUnrecordedHint")}</p>
      </div>
    </li>
  );
}
