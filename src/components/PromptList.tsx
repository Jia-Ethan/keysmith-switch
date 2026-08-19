import { useTranslation } from "react-i18next";
import type { PromptSummary } from "../types";
import { EmptyState } from "./EmptyState";
import { cx } from "./ui";

export function PromptList({
  prompts,
  selectedId,
  onSelect,
}: {
  prompts: PromptSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const { t } = useTranslation();
  const active = prompts.filter((item) => item.active);
  const inactive = prompts.filter((item) => !item.active);

  if (prompts.length === 0) {
    return <EmptyState title={t("prompts.empty")} hint={t("prompts.emptyHint")} />;
  }

  return (
    <div className="space-y-3" data-testid="prompt-list">
      <Group title={t("prompts.active")} items={active} selectedId={selectedId} onSelect={onSelect} />
      <Group title={t("prompts.inactive")} items={inactive} selectedId={selectedId} onSelect={onSelect} />
    </div>
  );
}

function Group({
  title,
  items,
  selectedId,
  onSelect,
}: {
  title: string;
  items: PromptSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <section>
      <h3 className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-ink-200">
        {title}
        <span className="ml-1 font-normal">({items.length})</span>
      </h3>
      {items.length === 0 ? (
        <p className="px-1 py-2 text-[12px] text-ink-200">—</p>
      ) : (
        <ul className="space-y-1">
          {items.map((item) => (
            <li key={item.id}>
              <button
                type="button"
                onClick={() => onSelect(item.id)}
                className={cx(
                  "w-full rounded px-2 py-1.5 text-left hover:bg-ink-700",
                  selectedId === item.id && "bg-ink-700 ring-1 ring-accent-600/60",
                )}
              >
                <div className="truncate text-[12px] font-medium">{item.title}</div>
                <div className="mt-0.5 flex flex-wrap gap-1 text-[10px] text-ink-200">
                  {item.tags.map((tag) => (
                    <span key={tag} className="rounded bg-ink-900 px-1">
                      {tag}
                    </span>
                  ))}
                  {item.lastUsedAt ? <span>{item.lastUsedAt}</span> : null}
                </div>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
