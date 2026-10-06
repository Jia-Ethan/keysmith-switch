import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { IconSwap } from "../components/icons";
import { Switch } from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import type { RewriteRule, RewriteView, RuleTable, ToolId } from "../types";
import { AgentList } from "./rewrite/AgentList";
import { AGENT_NAMES, SWITCH_KEY, agentRows } from "./rewrite/agents";
import { RuleTables } from "./rewrite/RuleTables";

function connectCall(tool: ToolId): (() => Promise<RewriteView>) | undefined {
  switch (tool) {
    case "codex":
      return api.rewriteConnectCodex;
    case "claude":
      return api.rewriteConnectClaude;
    case "zcode":
      return api.rewriteConnectZcode;
    default:
      return undefined;
  }
}

function disconnectCall(tool: ToolId): (() => Promise<RewriteView>) | undefined {
  switch (tool) {
    case "codex":
      return api.rewriteDisconnectCodex;
    case "claude":
      return api.rewriteDisconnectClaude;
    case "zcode":
      return api.rewriteDisconnectZcode;
    default:
      return undefined;
  }
}

/**
 * Input rewrite: the rules and switches live here; the replacing happens in a local relay
 * that connected agents send through. This page never sees a conversation.
 */
export function RewritePage({ toast }: { toast: ToastApi }) {
  const { t } = useTranslation();
  const [view, setView] = useState<RewriteView | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [connecting, setConnecting] = useState<ToolId | null>(null);
  const [disconnecting, setDisconnecting] = useState<ToolId | null>(null);
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

  const rows = useMemo(() => (view ? agentRows(view) : []), [view]);
  const packs = useMemo(() => view?.tables.filter((table) => table.kind === "pack") ?? [], [view]);
  const rowFor = (tool: ToolId | null) => rows.find((row) => row.tool === tool);

  const move = (index: number, delta: number) => {
    const ids = packs.map((table) => table.id);
    const target = index + delta;
    if (target < 0 || target >= ids.length) return;
    [ids[index], ids[target]] = [ids[target], ids[index]];
    void run("reorder", () => api.rewriteReorderTables(ids));
  };

  const connect = (tool: ToolId) => {
    const action = connectCall(tool);
    if (!action) return;
    void run(`connect:${tool}`, action, t("rewrite.connect.connected", { agent: AGENT_NAMES[tool] })).then(() =>
      setConnecting(null),
    );
  };

  const confirming = rowFor(connecting);
  const leaving = rowFor(disconnecting);

  return (
    <section className="h-full min-h-0 w-full overflow-y-auto" data-testid="rewrite-page">
      <div className="mx-auto flex w-full max-w-[960px] flex-col gap-6 px-4 pb-24 pt-6 sm:px-6">
        <header className="flex flex-wrap items-start gap-x-4 gap-y-2">
          <div className="min-w-0 flex-1">
            <h1 className="flex items-center gap-2 text-[22px] font-semibold tracking-[-0.02em] text-foreground">
              <IconSwap size={20} className="text-primary" />
              {t("rewrite.title")}
            </h1>
            <p className="mt-1 text-[13px] text-muted-foreground">{t("rewrite.lead")}</p>
          </div>
          {view ? (
            <div className="flex shrink-0 items-center gap-2 pt-1">
              <span className="text-[13px] text-muted-foreground" aria-hidden="true">
                {view.enabled ? t("rewrite.master.on") : t("rewrite.master.off")}
              </span>
              <Switch
                id="rewrite-master"
                data-testid="rewrite-master"
                aria-label={t("rewrite.master.label")}
                checked={view.enabled}
                disabled={busy !== null && busy !== "master"}
                busy={busy === "master"}
                onCheckedChange={(enabled) => void run("master", () => api.rewriteSetSwitches({ enabled }))}
              />
            </div>
          ) : null}
        </header>

        {!view ? (
          <div className="surface-card h-[140px] p-5" aria-hidden="true">
            <div className="skeleton h-4 w-1/3" />
            <div className="skeleton mt-3 h-3 w-2/3" />
          </div>
        ) : (
          <>
            {!view.enabled ? (
              <p className="-mt-2 rounded-xl border border-border bg-muted/50 px-4 py-2.5 text-[12.5px] text-muted-foreground" data-testid="rewrite-paused">
                {t("rewrite.master.pausedBanner")}
              </p>
            ) : null}

            <AgentList
              rows={rows}
              masterOn={view.enabled}
              busy={busy}
              onToggle={(tool, enabled) =>
                void run(`switch:${tool}`, () => api.rewriteSetSwitches({ [SWITCH_KEY[tool]]: enabled }))
              }
              onConnect={(tool) => {
                // A first connection says what it writes; repairing an existing one does not ask again.
                if (rowFor(tool)?.connected) connect(tool);
                else setConnecting(tool);
              }}
              onDisconnect={setDisconnecting}
            />

            <RuleTables
              tables={view.tables}
              busy={busy !== null}
              onToggle={(id, enabled) => void run("toggle", () => api.rewriteSetTableEnabled(id, enabled))}
              onSave={(rules: RewriteRule[]) => run("save", () => api.rewriteSaveUserRules(rules), t("rewrite.editor.saved"))}
              onScope={(id, tools) => void run("scope", () => api.rewriteSetTableTools(id, tools))}
              onMove={move}
              onCopy={(id) => void run("copy", () => api.rewriteCopyToUser(id), t("rewrite.copied"))}
              onReview={setReviewing}
            />
          </>
        )}
      </div>

      <ConfirmDialog
        open={confirming !== undefined}
        title={confirming ? t("rewrite.connect.title", { agent: AGENT_NAMES[confirming.tool] }) : ""}
        confirmLabel={t("rewrite.connect.confirm")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        confirmTestId="rewrite-connect-confirm"
        busy={busy === `connect:${connecting}`}
        onClose={() => setConnecting(null)}
        onConfirm={() => connecting && connect(connecting)}
      >
        {confirming ? (
          <div className="space-y-2 text-[13.5px] leading-relaxed text-muted-foreground">
            <p>
              {t("rewrite.connect.body", {
                agent: AGENT_NAMES[confirming.tool],
                file: confirming.file ?? "",
                field: confirming.field ?? t("rewrite.details.providerField"),
              })}
            </p>
            {confirming.hosts.length ? (
              <p>{t("rewrite.details.upstreams", { hosts: confirming.hosts.join(", ") })}</p>
            ) : null}
            {confirming.oauthNote ? <p>{t("rewrite.details.oauth")}</p> : null}
          </div>
        ) : null}
      </ConfirmDialog>

      <ConfirmDialog
        open={leaving !== undefined}
        danger
        title={leaving ? t("rewrite.connect.disconnectTitle", { agent: AGENT_NAMES[leaving.tool] }) : ""}
        confirmLabel={t("rewrite.action.disconnect").replace(/…$/, "")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        confirmTestId="rewrite-disconnect-confirm"
        busy={busy === `disconnect:${disconnecting}`}
        onClose={() => setDisconnecting(null)}
        onConfirm={() => {
          const tool = disconnecting;
          const action = tool ? disconnectCall(tool) : undefined;
          if (!tool || !action) return;
          void run(`disconnect:${tool}`, action, t("rewrite.connect.disconnected", { agent: AGENT_NAMES[tool] })).then(() =>
            setDisconnecting(null),
          );
        }}
      >
        {leaving ? (
          <p className="text-[13.5px] leading-relaxed text-muted-foreground">
            {t("rewrite.connect.disconnectBody", { agent: AGENT_NAMES[leaving.tool], file: leaving.file ?? "" })}
          </p>
        ) : null}
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
        <p className="mb-3 text-[13px] text-muted-foreground">
          {reviewing?.enabled ? t("rewrite.reviewBodyOn") : t("rewrite.reviewBodyOff")}
        </p>
        {reviewing?.pending ? <RuleDiff before={reviewing.rules} after={reviewing.pending.rules} /> : null}
      </ConfirmDialog>
    </section>
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
              <span>{rule.to || t("rewrite.editor.empty")}</span>
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
              <span className="line-through">{rule.to || t("rewrite.editor.empty")}</span>
            </div>
          ))}
          </div>
        </section>
      ) : null}
      {changed.length ? (
        <section>
          <h4 className="mb-1 text-[12px] font-semibold text-warning-strong">{t("rewrite.diffChanged")}</h4>
          <div className={grid}>
          {changed.map((rule) => (
            <div key={rule.from} className={row}>
              <span className="text-warning-strong">~</span>
              <span>{rule.from}</span>
              <span className="text-muted-foreground">→</span>
              <span>
                <span className="line-through opacity-60">{rule.before || t("rewrite.editor.empty")}</span> {rule.after || t("rewrite.editor.empty")}
              </span>
            </div>
          ))}
          </div>
        </section>
      ) : null}
    </div>
  );
}
