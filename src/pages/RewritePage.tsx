import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { Callout } from "../components/PlanPreview";
import { IconAlert, IconCheck, IconChevronDown, IconCopy, IconPlus, IconSwap, IconTrash } from "../components/icons";
import { ToolLogo } from "../components/ToolLogos";
import { Button, Switch, IconButton, Input, SettingRow, cx } from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import type { RewriteRule, RewriteView, RuleTable } from "../types";

/**
 * Input rewrite: the rules and switches live here; the replacing happens in a local relay
 * that Codex sends through. This page never sees a conversation.
 */
export function RewritePage({ toast }: { toast: ToastApi }) {
  const { t } = useTranslation();
  const [view, setView] = useState<RewriteView | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [disconnecting, setDisconnecting] = useState(false);
  const [reviewing, setReviewing] = useState<RuleTable | null>(null);

  const load = useCallback(async () => {
    try {
      setView(await api.rewriteState());
    } catch (error) {
      toast.err(error instanceof Error ? error.message : t("rewrite.loadError"));
    }
  }, [toast, t]);

  useEffect(() => {
    void load();
  }, [load]);

  const run = async (key: string, action: () => Promise<RewriteView>, done?: string) => {
    setBusy(key);
    try {
      setView(await action());
      if (done) toast.ok(done);
      return true;
    } catch (error) {
      toast.err(error);
      return false;
    } finally {
      setBusy(null);
    }
  };

  const packs = useMemo(() => view?.tables.filter((table) => table.kind === "pack") ?? [], [view]);
  const mine = view?.tables.find((table) => table.kind === "user");

  const move = (index: number, delta: number) => {
    const ids = packs.map((table) => table.id);
    const target = index + delta;
    if (target < 0 || target >= ids.length) return;
    [ids[index], ids[target]] = [ids[target], ids[index]];
    void run("reorder", () => api.rewriteReorderTables(ids));
  };

  return (
    <section className="h-full min-h-0 w-full overflow-y-auto" data-testid="rewrite-page">
      <div className="mx-auto flex w-full max-w-[960px] flex-col gap-5 px-4 pb-10 pt-6 sm:px-6">
        <header className="flex flex-wrap items-center gap-4">
          <span className="flex h-12 w-12 items-center justify-center rounded-2xl bg-primary/12 text-primary ring-1 ring-primary/20">
            <IconSwap size={24} />
          </span>
          <div className="min-w-0 flex-1">
            <h1 className="text-[22px] font-semibold tracking-[-0.02em] text-foreground">{t("rewrite.title")}</h1>
            <p className="mt-0.5 text-[13px] text-muted-foreground">{t("rewrite.lead")}</p>
          </div>
        </header>

        {!view ? (
          <div className="surface-card h-[140px] p-5" aria-hidden="true">
            <div className="skeleton h-4 w-1/3" />
            <div className="skeleton mt-3 h-3 w-2/3" />
          </div>
        ) : (
          <>
            <div className="surface-card">
              <SettingRow
                label={t("rewrite.masterLabel")}
                description={t("rewrite.masterHint")}
                htmlFor="rewrite-master"
                control={
                  <Switch
                    id="rewrite-master"
                    data-testid="rewrite-master"
                    checked={view.enabled}
                    disabled={busy !== null}
                    onCheckedChange={(enabled) => void run("switch", () => api.rewriteSetSwitches({ enabled }))}
                  />
                }
              />
              <SettingRow
                label={t("rewrite.codexLabel")}
                description={t("rewrite.codexHint")}
                htmlFor="rewrite-codex"
                control={
                  <Switch
                    id="rewrite-codex"
                    data-testid="rewrite-codex"
                    checked={view.codexEnabled}
                    disabled={busy !== null || !view.enabled}
                    onCheckedChange={(codexEnabled) => void run("switch", () => api.rewriteSetSwitches({ codexEnabled }))}
                  />
                }
              />
            </div>

            <CodexConnection
              view={view}
              busy={busy}
              onConnect={() => void run("connect", api.rewriteConnectCodex, t("rewrite.connect.connected"))}
              onDisconnect={() => setDisconnecting(true)}
            />

            <div className="flex flex-col gap-2">
              <div>
                <h2 className="text-[15px] font-semibold text-foreground">{t("rewrite.tables")}</h2>
                <p className="text-[12.5px] text-muted-foreground">{t("rewrite.tablesHint")}</p>
              </div>
              {mine ? (
                <UserTable
                  table={mine}
                  busy={busy !== null}
                  onToggle={(enabled) => void run("toggle", () => api.rewriteSetTableEnabled(mine.id, enabled))}
                  onSave={(rules) => run("save", () => api.rewriteSaveUserRules(rules), t("rewrite.editor.saved"))}
                />
              ) : null}
              {packs.map((table, index) => (
                <PackTable
                  key={table.id}
                  table={table}
                  busy={busy !== null}
                  first={index === 0}
                  last={index === packs.length - 1}
                  onToggle={(enabled) => void run("toggle", () => api.rewriteSetTableEnabled(table.id, enabled))}
                  onMove={(delta) => move(index, delta)}
                  onCopy={() => void run("copy", () => api.rewriteCopyToUser(table.id), t("rewrite.copied"))}
                  onReview={() => setReviewing(table)}
                />
              ))}
            </div>
          </>
        )}
      </div>

      <ConfirmDialog
        open={disconnecting}
        title={t("rewrite.connect.disconnectTitle")}
        confirmLabel={t("rewrite.connect.disconnect")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        confirmTestId="rewrite-disconnect-confirm"
        busy={busy === "disconnect"}
        onClose={() => setDisconnecting(false)}
        onConfirm={() => {
          void run("disconnect", api.rewriteDisconnectCodex, t("rewrite.connect.disconnected")).then(() =>
            setDisconnecting(false),
          );
        }}
      >
        <p className="text-[13.5px] leading-relaxed text-muted-foreground">
          {t("rewrite.connect.disconnectBody", {
            provider: view?.codex.link.state === "linked" ? view.codex.link.provider : view?.codex.provider?.name ?? "",
          })}
        </p>
      </ConfirmDialog>

      <ConfirmDialog
        open={reviewing !== null}
        wide
        title={t("rewrite.reviewTitle", { title: reviewing?.title ?? "" })}
        confirmLabel={t("rewrite.accept")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        confirmTestId="rewrite-accept"
        busy={busy === "accept"}
        onClose={() => setReviewing(null)}
        onConfirm={() => {
          const table = reviewing;
          if (!table) return;
          void run("accept", () => api.rewriteAcceptUpdate(table.id), t("rewrite.accepted")).then(() => setReviewing(null));
        }}
      >
        <p className="mb-3 text-[13px] text-muted-foreground">{t("rewrite.reviewBody")}</p>
        {reviewing?.pending ? <RuleDiff before={reviewing.rules} after={reviewing.pending.rules} /> : null}
      </ConfirmDialog>
    </section>
  );
}

function CodexConnection({
  view,
  busy,
  onConnect,
  onDisconnect,
}: {
  view: RewriteView;
  busy: string | null;
  onConnect: () => void;
  onDisconnect: () => void;
}) {
  const { t } = useTranslation();
  const { link, service, provider, unsupported } = view.codex;
  const linked = link.state === "linked";
  const down = linked && !service.running;
  const canConnect = !unsupported || linked;

  return (
    <div className="surface-card flex flex-col gap-3 px-5 py-4" data-testid="rewrite-codex-connection" data-state={link.state}>
      <div className="flex flex-wrap items-center gap-3">
        <ToolLogo tool="codex" size={28} />
        <div className="min-w-0 flex-1">
          <h2 className="text-[14px] font-medium text-foreground">{t("rewrite.connect.title")}</h2>
          {provider && !linked ? (
            <p className="text-[12.5px] text-muted-foreground">{t("rewrite.connect.provider", { name: provider.name })}</p>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {linked && !down ? null : (
            <Button
              size="sm"
              variant="primary"
              data-testid="rewrite-connect"
              loading={busy === "connect"}
              disabled={busy !== null || !canConnect}
              onClick={onConnect}
            >
              {link.state === "unlinked" ? t("rewrite.connect.connectButton") : t("rewrite.connect.reconnect")}
            </Button>
          )}
          {link.state !== "unlinked" || service.installed ? (
            <Button size="sm" variant="ghost" data-testid="rewrite-disconnect" disabled={busy !== null} onClick={onDisconnect}>
              {t("rewrite.connect.disconnect")}
            </Button>
          ) : null}
        </div>
      </div>
      {linked && !down ? (
        <Callout tone="info" icon={<IconCheck size={14} />}>
          <span data-testid="rewrite-linked">{t("rewrite.connect.linked", { provider: link.provider })}</span>
        </Callout>
      ) : null}
      {down ? (
        <Callout tone="danger" icon={<IconAlert size={14} />}>
          <span data-testid="rewrite-down">{t("rewrite.connect.notRunning")}</span>
        </Callout>
      ) : null}
      {link.state === "bypassed" ? (
        <Callout tone="warn" icon={<IconAlert size={14} />}>
          <span data-testid="rewrite-bypassed">{t("rewrite.connect.bypassed")}</span>
        </Callout>
      ) : null}
      {link.state === "unlinked" ? <p className="text-[12.5px] leading-relaxed text-muted-foreground">{t("rewrite.connect.unlinked")}</p> : null}
      {unsupported && !linked ? (
        <Callout tone="warn" icon={<IconAlert size={14} />}>
          <span data-testid="rewrite-unsupported">{t(`rewrite.connect.unsupported.${unsupported}`, { defaultValue: unsupported })}</span>
        </Callout>
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
  onToggle,
  onSave,
}: {
  table: RuleTable;
  busy: boolean;
  onToggle: (enabled: boolean) => void;
  onSave: (rules: RewriteRule[]) => Promise<boolean>;
}) {
  const { t } = useTranslation();
  const toDrafts = (rules: RewriteRule[]) => rules.map((rule, index) => ({ ...rule, key: index }));
  const [drafts, setDrafts] = useState<Draft[]>(() => toDrafts(table.rules));
  const [nextKey, setNextKey] = useState(table.rules.length);
  const saved = JSON.stringify(table.rules);
  const current = drafts.map(({ from, to }) => ({ from, to }));
  const dirty = JSON.stringify(current) !== saved;
  const problem = validate(current, t);

  useEffect(() => {
    setDrafts(toDrafts(JSON.parse(saved) as RewriteRule[]));
  }, [saved]);

  const update = (key: number, patch: Partial<RewriteRule>) =>
    setDrafts((list) => list.map((draft) => (draft.key === key ? { ...draft, ...patch } : draft)));

  return (
    <div className="surface-card" data-testid="rewrite-table-user">
      <TableHeader
        title={t("rewrite.mine")}
        count={table.rules.length}
        enabled={table.enabled}
        busy={busy}
        onToggle={onToggle}
        testId="rewrite-toggle-user"
      />
      <div className="flex flex-col gap-2 px-5 py-4">
        {drafts.length === 0 ? (
          <p className="text-[12.5px] leading-relaxed text-muted-foreground" data-testid="rewrite-empty">
            {t("rewrite.editor.empty")}
          </p>
        ) : (
          <div className="grid grid-cols-[1fr_auto_1fr_auto] items-center gap-2">
            <span className="text-[11.5px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">{t("rewrite.editor.from")}</span>
            <span />
            <span className="text-[11.5px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">{t("rewrite.editor.to")}</span>
            <span />
            {drafts.map((draft, index) => (
              <RuleRow
                key={draft.key}
                index={index}
                draft={draft}
                onChange={(patch) => update(draft.key, patch)}
                onRemove={() => setDrafts((list) => list.filter((item) => item.key !== draft.key))}
              />
            ))}
          </div>
        )}
        <div className="flex flex-wrap items-center gap-2 pt-1">
          <Button
            size="sm"
            variant="outline"
            data-testid="rewrite-add"
            onClick={() => {
              setDrafts((list) => [...list, { from: "", to: "", key: nextKey }]);
              setNextKey((key) => key + 1);
            }}
          >
            <IconPlus size={13} />
            {t("rewrite.editor.add")}
          </Button>
          {dirty ? (
            <>
              <span className="ml-auto text-[12px] text-muted-foreground" data-testid="rewrite-problem">
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
      </div>
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
      <IconButton label={t("rewrite.editor.remove")} onClick={onRemove} data-testid={`rewrite-remove-${index}`}>
        <IconTrash size={14} />
      </IconButton>
    </>
  );
}

function TableHeader({
  title,
  subtitle,
  count,
  enabled,
  busy,
  onToggle,
  testId,
  children,
}: {
  title: string;
  subtitle?: string;
  count: number;
  enabled: boolean;
  busy: boolean;
  onToggle: (enabled: boolean) => void;
  testId: string;
  children?: React.ReactNode;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex min-h-[48px] flex-wrap items-center gap-2 border-b border-border px-5 py-2.5">
      <div className="min-w-0 flex-1">
        <h3 className="truncate text-[14px] font-semibold text-foreground">{title}</h3>
        <p className="text-[12px] text-muted-foreground">
          {subtitle ? `${subtitle} · ` : ""}
          {t("rewrite.ruleCount", { count })}
        </p>
      </div>
      {children}
      {count > 0 ? (
        <Switch
          aria-label={t("rewrite.enableTable", { table: title })}
          checked={enabled}
          disabled={busy}
          data-testid={testId}
          onCheckedChange={onToggle}
        />
      ) : null}
    </div>
  );
}

function PackTable({
  table,
  busy,
  first,
  last,
  onToggle,
  onMove,
  onCopy,
  onReview,
}: {
  table: RuleTable;
  busy: boolean;
  first: boolean;
  last: boolean;
  onToggle: (enabled: boolean) => void;
  onMove: (delta: number) => void;
  onCopy: () => void;
  onReview: () => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <div className="surface-card" data-testid={`rewrite-table-${table.packId}`}>
      <TableHeader
        title={table.title || table.packId || ""}
        subtitle={`${t("rewrite.pack")}${table.packVersion ? ` v${table.packVersion}` : ""}`}
        count={table.rules.length}
        enabled={table.enabled}
        busy={busy}
        onToggle={onToggle}
        testId={`rewrite-toggle-${table.packId}`}
      >
        <IconButton label={t("rewrite.moveUp")} disabled={busy || first} onClick={() => onMove(-1)}>
          <IconChevronDown size={14} className="rotate-180" />
        </IconButton>
        <IconButton label={t("rewrite.moveDown")} disabled={busy || last} onClick={() => onMove(1)}>
          <IconChevronDown size={14} />
        </IconButton>
        <IconButton label={t("rewrite.copyToMine")} disabled={busy} onClick={onCopy} data-testid={`rewrite-copy-${table.packId}`}>
          <IconCopy size={14} />
        </IconButton>
      </TableHeader>
      {table.pending ? (
        <div className="flex flex-wrap items-center gap-2 border-b border-border bg-warning/10 px-5 py-2 text-[12.5px] text-warning">
          <span className="flex-1" data-testid={`rewrite-pending-${table.packId}`}>
            {t("rewrite.pending", { version: table.pending.version })}
          </span>
          <Button size="sm" variant="outline" disabled={busy} onClick={onReview} data-testid={`rewrite-review-${table.packId}`}>
            {t("rewrite.review")}
          </Button>
        </div>
      ) : null}
      <button
        type="button"
        className="flex w-full items-center gap-2 px-5 py-2.5 text-left text-[12.5px] text-muted-foreground hover:text-foreground"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <IconChevronDown size={13} className={cx("transition-transform", open ? "" : "-rotate-90")} />
        {t("rewrite.readOnly")}
      </button>
      {open ? (
        <ul className="grid grid-cols-[1fr_auto_1fr] gap-x-3 gap-y-1 px-5 pb-4 font-mono text-[12.5px]">
          {table.rules.map((rule) => (
            <li key={rule.from} className="contents">
              <span className="truncate text-foreground">{rule.from}</span>
              <span className="text-muted-foreground">→</span>
              <span className="truncate text-foreground">{rule.to || "∅"}</span>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

export function ruleDiff(before: RewriteRule[], after: RewriteRule[]) {
  const old = new Map(before.map((rule) => [rule.from, rule.to]));
  const next = new Map(after.map((rule) => [rule.from, rule.to]));
  const added = after.filter((rule) => !old.has(rule.from));
  const removed = before.filter((rule) => !next.has(rule.from));
  const changed = after
    .filter((rule) => old.has(rule.from) && old.get(rule.from) !== rule.to)
    .map((rule) => ({ from: rule.from, before: old.get(rule.from) ?? "", after: rule.to }));
  return { added, removed, changed };
}

function RuleDiff({ before, after }: { before: RewriteRule[]; after: RewriteRule[] }) {
  const { t } = useTranslation();
  const { added, removed, changed } = ruleDiff(before, after);
  if (!added.length && !removed.length && !changed.length) {
    return <p className="text-[13px] text-muted-foreground">{t("rewrite.noChanges")}</p>;
  }
  const grid = "grid grid-cols-[1rem_minmax(0,max-content)_auto_minmax(0,1fr)] items-baseline gap-x-2 gap-y-0.5 font-mono text-[12.5px]";
  const row = "contents";
  return (
    <div className="flex max-h-[50vh] flex-col gap-3 overflow-y-auto" data-testid="rewrite-diff">
      {added.length ? (
        <section>
          <h4 className="mb-1 text-[12px] font-semibold text-success">{t("rewrite.diffAdded")}</h4>
          <div className={grid}>
          {added.map((rule) => (
            <div key={rule.from} className={row}>
              <span className="text-success">+</span>
              <span>{rule.from}</span>
              <span className="text-muted-foreground">→</span>
              <span>{rule.to || "∅"}</span>
            </div>
          ))}
          </div>
        </section>
      ) : null}
      {removed.length ? (
        <section>
          <h4 className="mb-1 text-[12px] font-semibold text-destructive">{t("rewrite.diffRemoved")}</h4>
          <div className={grid}>
          {removed.map((rule) => (
            <div key={rule.from} className={row}>
              <span className="text-destructive">−</span>
              <span className="line-through">{rule.from}</span>
              <span className="text-muted-foreground">→</span>
              <span className="line-through">{rule.to || "∅"}</span>
            </div>
          ))}
          </div>
        </section>
      ) : null}
      {changed.length ? (
        <section>
          <h4 className="mb-1 text-[12px] font-semibold text-warning">{t("rewrite.diffChanged")}</h4>
          <div className={grid}>
          {changed.map((rule) => (
            <div key={rule.from} className={row}>
              <span className="text-warning">~</span>
              <span>{rule.from}</span>
              <span className="text-muted-foreground">→</span>
              <span>
                <span className="line-through opacity-60">{rule.before || "∅"}</span> {rule.after || "∅"}
              </span>
            </div>
          ))}
          </div>
        </section>
      ) : null}
    </div>
  );
}
