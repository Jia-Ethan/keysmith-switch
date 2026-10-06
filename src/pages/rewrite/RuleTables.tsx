import { useEffect, useId, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { IconChevronDown, IconCopy, IconInfo, IconPlus, IconSearch, IconTrash } from "../../components/icons";
import { ToolLogo } from "../../components/ToolLogos";
import { Button, IconButton, Input, Switch, cx } from "../../components/ui";
import { Dropdown } from "../../components/Dropdown";
import type { RewriteRule, RuleTable, ToolId } from "../../types";
import { AGENT_NAMES, REWRITE_AGENTS } from "./agents";

/** Rows shown before a long table folds. */
const FOLD_AT = 20;

type Filter = "all" | ToolId;
type Sort = "added" | "from";

function appliesTo(table: RuleTable, tool: Filter) {
  return tool === "all" || table.tools === null || table.tools.includes(tool);
}

function matches(rule: RewriteRule, query: string) {
  if (!query) return true;
  const q = query.toLowerCase();
  return rule.from.toLowerCase().includes(q) || rule.to.toLowerCase().includes(q);
}

function sorted<T extends RewriteRule>(rules: T[], sort: Sort): T[] {
  return sort === "from" ? [...rules].sort((a, b) => a.from.localeCompare(b.from)) : rules;
}

const COLLAPSED_KEY = "keysmith-switch.rewrite.collapsed";

function readCollapsed(): string[] {
  try {
    const value = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
    return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
  } catch {
    return [];
  }
}

export function RuleTables({
  tables,
  busy,
  onToggle,
  onSave,
  onScope,
  onMove,
  onCopy,
  onReview,
}: {
  tables: RuleTable[];
  busy: boolean;
  onToggle: (id: string, enabled: boolean) => void;
  onSave: (rules: RewriteRule[]) => Promise<boolean>;
  onScope: (id: string, tools: ToolId[] | null) => void;
  onMove: (index: number, delta: number) => void;
  onCopy: (id: string) => void;
  onReview: (table: RuleTable) => void;
}) {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("added");
  const [collapsed, setCollapsed] = useState<string[]>(readCollapsed);
  const [howOpen, setHowOpen] = useState(false);
  const howId = useId();

  useEffect(() => {
    try {
      localStorage.setItem(COLLAPSED_KEY, JSON.stringify(collapsed));
    } catch {
      // Folding is a convenience; it simply isn't remembered.
    }
  }, [collapsed]);

  const mine = tables.find((table) => table.kind === "user");
  const packs = tables.filter((table) => table.kind === "pack");
  const visiblePacks = packs.filter((table) => appliesTo(table, filter));
  const total = tables.reduce((sum, table) => sum + table.rules.length, 0);
  const toggleCollapsed = (id: string) =>
    setCollapsed((list) => (list.includes(id) ? list.filter((item) => item !== id) : [...list, id]));

  return (
    <section className="flex flex-col gap-2" aria-labelledby="rewrite-rules-heading">
      <div className="flex flex-col gap-2">
        <div className="min-w-0">
          <h2 id="rewrite-rules-heading" className="flex items-center gap-1.5 text-[15px] font-semibold text-foreground">
            {t("rewrite.rules.heading")}
            <button
              type="button"
              aria-expanded={howOpen}
              aria-controls={howId}
              data-testid="rewrite-how"
              onClick={() => setHowOpen((value) => !value)}
              className="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[12px] font-normal text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <IconInfo size={12} />
              {t("rewrite.rules.howItMatches")}
            </button>
          </h2>
          <p className="text-[12.5px] text-muted-foreground">{t("rewrite.rules.hint")}</p>
        </div>
        {total > 0 ? (
          <div className="flex flex-wrap items-center gap-2" role="toolbar" aria-label={t("rewrite.rules.heading")}>
            <Dropdown<Filter>
              label={t("rewrite.rules.filter")}
              value={filter}
              testId="rewrite-filter"
              options={[
                { value: "all", label: t("rewrite.rules.filterAll") },
                ...REWRITE_AGENTS.map((tool) => ({ value: tool as Filter, label: AGENT_NAMES[tool] })),
              ]}
              onChange={setFilter}
            />
            <label className="relative">
              <span className="sr-only">{t("rewrite.rules.search")}</span>
              <IconSearch size={13} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={query}
                placeholder={t("rewrite.rules.search")}
                data-testid="rewrite-search"
                className="h-8 w-[180px] pl-7 text-[12.5px]"
                onChange={(event) => setQuery(event.target.value)}
              />
            </label>
            <Dropdown<Sort>
              label={t("rewrite.rules.sort")}
              value={sort}
              testId="rewrite-sort"
              options={[
                { value: "added", label: t("rewrite.rules.sortAdded") },
                { value: "from", label: t("rewrite.rules.sortFrom") },
              ]}
              onChange={setSort}
            />
          </div>
        ) : null}
      </div>
      {howOpen ? (
        <p id={howId} className="rounded-xl bg-muted/60 px-3 py-2 text-[12.5px] leading-relaxed text-muted-foreground" data-testid="rewrite-how-body">
          {t("rewrite.rules.howBody")}
        </p>
      ) : null}

      {mine && appliesTo(mine, filter) ? (
        <UserTable
          table={mine}
          busy={busy}
          query={query}
          sort={sort}
          collapsed={collapsed.includes(mine.id)}
          onCollapse={() => toggleCollapsed(mine.id)}
          onToggle={(enabled) => onToggle(mine.id, enabled)}
          onSave={onSave}
          onScope={(tools) => onScope(mine.id, tools)}
        />
      ) : null}
      {visiblePacks.map((table) => {
        const index = packs.indexOf(table);
        return (
          <PackTable
            key={table.id}
            table={table}
            busy={busy}
            query={query}
            sort={sort}
            first={index === 0}
            last={index === packs.length - 1}
            collapsed={collapsed.includes(table.id)}
            onCollapse={() => toggleCollapsed(table.id)}
            onToggle={(enabled) => onToggle(table.id, enabled)}
            onScope={(tools) => onScope(table.id, tools)}
            onMove={(delta) => onMove(index, delta)}
            onCopy={() => onCopy(table.id)}
            onReview={() => onReview(table)}
          />
        );
      })}
    </section>
  );
}

/** Which agents a table applies to. All four selected is stored as "every agent". */
function ScopeChips({
  table,
  title,
  busy,
  onScope,
}: {
  table: RuleTable;
  title: string;
  busy: boolean;
  onScope: (tools: ToolId[] | null) => void;
}) {
  const { t } = useTranslation();
  const selected = table.tools ?? REWRITE_AGENTS;
  const [hint, setHint] = useState(false);
  return (
    <div className="flex flex-wrap items-center gap-1.5 px-5 pb-1 pt-2" role="group" aria-label={t("rewrite.rules.scopeLabel", { table: title })}>
      <span className="text-[12px] text-muted-foreground">{t("rewrite.rules.scope")}</span>
      {REWRITE_AGENTS.map((tool) => {
        const on = selected.includes(tool);
        return (
          <button
            key={tool}
            type="button"
            aria-pressed={on}
            disabled={busy}
            data-testid={`rewrite-scope-${table.id}-${tool}`}
            onClick={() => {
              const next = on ? selected.filter((item) => item !== tool) : [...selected, tool];
              if (next.length === 0) {
                setHint(true);
                return;
              }
              setHint(false);
              onScope(next.length === REWRITE_AGENTS.length ? null : REWRITE_AGENTS.filter((item) => next.includes(item)));
            }}
            className={cx(
              "inline-flex h-6 items-center gap-1 rounded-full border px-2 text-[11.5px] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50",
              on ? "border-primary/40 bg-primary/10 text-foreground" : "border-border text-muted-foreground opacity-70 hover:opacity-100",
            )}
          >
            <ToolLogo tool={tool} size={12} />
            {AGENT_NAMES[tool]}
          </button>
        );
      })}
      {hint ? <span className="text-[12px] text-destructive-strong" role="status">{t("rewrite.rules.scopeEmpty")}</span> : null}
    </div>
  );
}

function TableHeader({
  title,
  subtitle,
  count,
  enabled,
  busy,
  collapsed,
  onCollapse,
  onToggle,
  testId,
  children,
}: {
  title: string;
  subtitle?: string;
  count: number;
  enabled: boolean;
  busy: boolean;
  collapsed: boolean;
  onCollapse: () => void;
  onToggle: (enabled: boolean) => void;
  testId: string;
  children?: React.ReactNode;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex min-h-[52px] flex-wrap items-center gap-2 px-5 py-2.5">
      <button
        type="button"
        aria-expanded={!collapsed}
        aria-label={collapsed ? t("rewrite.rules.expand", { table: title }) : t("rewrite.rules.collapse", { table: title })}
        onClick={onCollapse}
        className="-ml-1.5 flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <IconChevronDown size={14} className={cx("transition-transform", collapsed && "-rotate-90")} />
      </button>
      <div className="min-w-0 flex-1">
        <h3 className="truncate text-[14px] font-semibold text-foreground">{title}</h3>
        <p className="text-[12px] text-muted-foreground">
          {subtitle ? `${subtitle} · ` : ""}
          {t("rewrite.rules.count", { count })}
        </p>
      </div>
      {children}
      {count > 0 ? (
        <Switch
          aria-label={t("rewrite.rules.enableTable", { table: title })}
          checked={enabled}
          disabled={busy}
          data-testid={testId}
          onCheckedChange={onToggle}
        />
      ) : null}
    </div>
  );
}

type Draft = RewriteRule & { key: number };

function validate(rules: RewriteRule[], t: (key: string, options?: Record<string, unknown>) => string): string | null {
  const seen = new Set<string>();
  for (const rule of rules) {
    if (!rule.from) return t("rewrite.editor.emptyFrom");
    if (seen.has(rule.from)) return t("rewrite.editor.duplicate", { from: rule.from });
    seen.add(rule.from);
  }
  return null;
}

function UserTable({
  table,
  busy,
  query,
  sort,
  collapsed,
  onCollapse,
  onToggle,
  onSave,
  onScope,
}: {
  table: RuleTable;
  busy: boolean;
  query: string;
  sort: Sort;
  collapsed: boolean;
  onCollapse: () => void;
  onToggle: (enabled: boolean) => void;
  onSave: (rules: RewriteRule[]) => Promise<boolean>;
  onScope: (tools: ToolId[] | null) => void;
}) {
  const { t } = useTranslation();
  const toDrafts = (rules: RewriteRule[]) => rules.map((rule, index) => ({ ...rule, key: index }));
  const [drafts, setDrafts] = useState<Draft[]>(() => toDrafts(table.rules));
  const [nextKey, setNextKey] = useState(table.rules.length);
  const [showAll, setShowAll] = useState(false);
  const saved = JSON.stringify(table.rules);
  const current = drafts.map(({ from, to }) => ({ from, to }));
  const dirty = JSON.stringify(current) !== saved;
  const problem = validate(current, t);
  const title = t("rewrite.mine");

  useEffect(() => {
    setDrafts(toDrafts(JSON.parse(saved) as RewriteRule[]));
  }, [saved]);

  const update = (key: number, patch: Partial<RewriteRule>) =>
    setDrafts((list) => list.map((draft) => (draft.key === key ? { ...draft, ...patch } : draft)));
  const add = () => {
    setDrafts((list) => [...list, { from: "", to: "", key: nextKey }]);
    setNextKey((key) => key + 1);
    setShowAll(true);
  };

  // New, still-empty rows always show so the person can fill them in.
  const filtered = useMemo(
    () => sorted(drafts.filter((draft) => !draft.from || matches(draft, query)), sort),
    [drafts, query, sort],
  );
  const shown = showAll || query ? filtered : filtered.slice(0, FOLD_AT);

  return (
    <div className="surface-card" data-testid="rewrite-table-user">
      <TableHeader
        title={title}
        count={table.rules.length}
        enabled={table.enabled}
        busy={busy}
        collapsed={collapsed}
        onCollapse={onCollapse}
        onToggle={onToggle}
        testId="rewrite-toggle-user"
      />
      {collapsed ? null : (
        <div className="border-t border-border">
          {table.rules.length > 0 ? <ScopeChips table={table} title={title} busy={busy} onScope={onScope} /> : null}
          <div className="flex flex-col gap-2 px-5 py-3">
            {drafts.length === 0 ? (
              <div className="flex flex-wrap items-center gap-3 py-1" data-testid="rewrite-empty">
                <p className="text-[13px] text-muted-foreground">{t("rewrite.rules.empty")}</p>
                <Button size="sm" variant="subtle" data-testid="rewrite-add" onClick={add}>
                  <IconPlus size={13} />
                  {t("rewrite.rules.addFirst")}
                </Button>
              </div>
            ) : (
              <>
                {shown.length === 0 ? (
                  <p className="text-[12.5px] text-muted-foreground">{t("rewrite.rules.noMatch")}</p>
                ) : (
                  <div className="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)_auto] items-center gap-x-2 gap-y-2">
                    <span className="text-[11.5px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">{t("rewrite.editor.from")}</span>
                    <span />
                    <span className="text-[11.5px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">{t("rewrite.editor.to")}</span>
                    <span />
                    {shown.map((draft) => {
                      const index = drafts.indexOf(draft);
                      return (
                        <RuleRow
                          key={draft.key}
                          index={index}
                          draft={draft}
                          onChange={(patch) => update(draft.key, patch)}
                          onRemove={() => setDrafts((list) => list.filter((item) => item.key !== draft.key))}
                        />
                      );
                    })}
                  </div>
                )}
                {!query && filtered.length > FOLD_AT ? (
                  <button
                    type="button"
                    className="self-start text-[12.5px] text-primary hover:underline"
                    data-testid="rewrite-show-all"
                    onClick={() => setShowAll((value) => !value)}
                  >
                    {showAll ? t("rewrite.rules.showLess") : t("rewrite.rules.showAll", { count: filtered.length })}
                  </button>
                ) : null}
                <div className="flex flex-wrap items-center gap-2 pt-1">
                  <Button size="sm" variant="outline" data-testid="rewrite-add" onClick={add}>
                    <IconPlus size={13} />
                    {t("rewrite.editor.add")}
                  </Button>
                  {dirty ? (
                    <>
                      <span
                        className={cx("ml-auto text-[12px]", problem ? "text-destructive-strong" : "text-muted-foreground")}
                        role={problem ? "alert" : undefined}
                        data-testid="rewrite-problem"
                      >
                        {problem ?? t("rewrite.editor.unsaved")}
                      </span>
                      <Button size="sm" variant="ghost" disabled={busy} onClick={() => setDrafts(toDrafts(table.rules))}>
                        {t("rewrite.editor.discard")}
                      </Button>
                      <Button
                        size="sm"
                        variant="primary"
                        data-testid="rewrite-save"
                        disabled={busy || problem !== null}
                        onClick={() => void onSave(current)}
                      >
                        {t("rewrite.editor.save")}
                      </Button>
                    </>
                  ) : null}
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function RuleRow({
  index,
  draft,
  onChange,
  onRemove,
}: {
  index: number;
  draft: Draft;
  onChange: (patch: Partial<RewriteRule>) => void;
  onRemove: () => void;
}) {
  const { t } = useTranslation();
  return (
    <>
      <Input
        value={draft.from}
        aria-label={`${t("rewrite.editor.from")} ${index + 1}`}
        placeholder={t("rewrite.editor.fromPlaceholder")}
        data-testid={`rewrite-from-${index}`}
        onChange={(event) => onChange({ from: event.target.value.replace(/[\r\n\t]/g, "") })}
      />
      <span className="text-muted-foreground" aria-hidden="true">
        →
      </span>
      <Input
        value={draft.to}
        aria-label={`${t("rewrite.editor.to")} ${index + 1}`}
        placeholder={t("rewrite.editor.toPlaceholder")}
        data-testid={`rewrite-to-${index}`}
        onChange={(event) => onChange({ to: event.target.value })}
      />
      <IconButton label={t("rewrite.editor.remove", { n: index + 1 })} onClick={onRemove} data-testid={`rewrite-remove-${index}`}>
        <IconTrash size={14} />
      </IconButton>
    </>
  );
}

function PackTable({
  table,
  busy,
  query,
  sort,
  first,
  last,
  collapsed,
  onCollapse,
  onToggle,
  onScope,
  onMove,
  onCopy,
  onReview,
}: {
  table: RuleTable;
  busy: boolean;
  query: string;
  sort: Sort;
  first: boolean;
  last: boolean;
  collapsed: boolean;
  onCollapse: () => void;
  onToggle: (enabled: boolean) => void;
  onScope: (tools: ToolId[] | null) => void;
  onMove: (delta: number) => void;
  onCopy: () => void;
  onReview: () => void;
}) {
  const { t } = useTranslation();
  const [showAll, setShowAll] = useState(false);
  const title = table.title || table.packId || "";
  const rules = sorted(table.rules.filter((rule) => matches(rule, query)), sort);
  const shown = showAll || query ? rules : rules.slice(0, FOLD_AT);
  return (
    <div className="surface-card" data-testid={`rewrite-table-${table.packId}`}>
      <TableHeader
        title={title}
        subtitle={`${t("rewrite.pack")}${table.packVersion ? ` v${table.packVersion}` : ""}`}
        count={table.rules.length}
        enabled={table.enabled}
        busy={busy}
        collapsed={collapsed}
        onCollapse={onCollapse}
        onToggle={onToggle}
        testId={`rewrite-toggle-${table.packId}`}
      >
        <IconButton label={t("rewrite.moveUp", { table: title })} disabled={busy || first} onClick={() => onMove(-1)}>
          <IconChevronDown size={14} className="rotate-180" />
        </IconButton>
        <IconButton label={t("rewrite.moveDown", { table: title })} disabled={busy || last} onClick={() => onMove(1)}>
          <IconChevronDown size={14} />
        </IconButton>
        <Button
          size="xs"
          variant="ghost"
          title={t("rewrite.copyHint")}
          disabled={busy}
          onClick={onCopy}
          data-testid={`rewrite-copy-${table.packId}`}
        >
          <IconCopy size={13} />
          <span className="hidden sm:inline">{t("rewrite.copyToMine")}</span>
        </Button>
      </TableHeader>
      {table.pending ? (
        <div className="flex flex-wrap items-center gap-2 border-t border-border bg-warning/10 px-5 py-2 text-[12.5px] text-warning-strong">
          <span className="flex-1" data-testid={`rewrite-pending-${table.packId}`}>
            {t("rewrite.pending", { version: table.pending.version })}
          </span>
          <Button size="sm" variant="outline" disabled={busy} onClick={onReview} data-testid={`rewrite-review-${table.packId}`}>
            {t("rewrite.review")}
          </Button>
        </div>
      ) : null}
      {collapsed ? null : (
        <div className="border-t border-border">
          <ScopeChips table={table} title={title} busy={busy} onScope={onScope} />
          <p className="px-5 pt-1 text-[12px] text-muted-foreground">{t("rewrite.readOnly")}</p>
          {shown.length === 0 ? (
            <p className="px-5 py-3 text-[12.5px] text-muted-foreground">{t("rewrite.rules.noMatch")}</p>
          ) : (
            <ul className="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] gap-x-3 gap-y-1 px-5 py-3 font-mono text-[12.5px]" data-testid={`rewrite-pack-rules-${table.packId}`}>
              {shown.map((rule, index) => (
                <li key={`${index}:${rule.from}`} className="contents">
                  <span className="truncate text-foreground" title={rule.from}>
                    {rule.from}
                  </span>
                  <span className="text-muted-foreground">→</span>
                  <span className={cx("truncate", rule.to ? "text-foreground" : "italic text-muted-foreground")} title={rule.to}>
                    {rule.to || t("rewrite.editor.empty")}
                  </span>
                </li>
              ))}
            </ul>
          )}
          {!query && rules.length > FOLD_AT ? (
            <button
              type="button"
              className="mb-3 ml-5 text-[12.5px] text-primary hover:underline"
              onClick={() => setShowAll((value) => !value)}
            >
              {showAll ? t("rewrite.rules.showLess") : t("rewrite.rules.showAll", { count: rules.length })}
            </button>
          ) : null}
        </div>
      )}
    </div>
  );
}
