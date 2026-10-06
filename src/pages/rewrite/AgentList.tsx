import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { IconChevronDown } from "../../components/icons";
import { ToolLogo } from "../../components/ToolLogos";
import { Button, Switch, cx } from "../../components/ui";
import type { ToolId } from "../../types";
import { AGENT_NAMES, type AgentRow, type AgentStatus } from "./agents";

const DOT: Record<AgentStatus, string> = {
  active: "bg-success",
  paused: "bg-muted-foreground/60",
  unlinked: "border border-muted-foreground/60 bg-transparent",
  bypassed: "bg-warning",
  down: "bg-destructive",
  unsupported: "bg-muted-foreground/30",
};

const TEXT: Record<AgentStatus, string> = {
  active: "text-success-strong",
  paused: "text-muted-foreground",
  unlinked: "text-muted-foreground",
  bypassed: "text-warning-strong",
  down: "text-destructive-strong",
  unsupported: "text-muted-foreground",
};

export function AgentList({
  rows,
  masterOn,
  busy,
  onToggle,
  onConnect,
  onDisconnect,
}: {
  rows: AgentRow[];
  masterOn: boolean;
  busy: string | null;
  onToggle: (tool: ToolId, enabled: boolean) => void;
  onConnect: (tool: ToolId) => void;
  onDisconnect: (tool: ToolId) => void;
}) {
  const { t } = useTranslation();
  return (
    <section className="flex flex-col gap-2" aria-labelledby="rewrite-agents-heading">
      <div>
        <h2 id="rewrite-agents-heading" className="text-[15px] font-semibold text-foreground">
          {t("rewrite.agents.heading")}
        </h2>
        <p className="text-[12.5px] text-muted-foreground">{t("rewrite.agents.hint")}</p>
      </div>
      <ul className="surface-card divide-y divide-border/70" data-testid="rewrite-agents">
        {rows.map((row) => (
          <AgentRowView
            key={row.tool}
            row={row}
            masterOn={masterOn}
            busy={busy}
            onToggle={(enabled) => onToggle(row.tool, enabled)}
            onConnect={() => onConnect(row.tool)}
            onDisconnect={() => onDisconnect(row.tool)}
          />
        ))}
      </ul>
    </section>
  );
}

function AgentRowView({
  row,
  masterOn,
  busy,
  onToggle,
  onConnect,
  onDisconnect,
}: {
  row: AgentRow;
  masterOn: boolean;
  busy: string | null;
  onToggle: (enabled: boolean) => void;
  onConnect: () => void;
  onDisconnect: () => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const detailsId = useId();
  const name = AGENT_NAMES[row.tool];
  const rowBusy = busy === `connect:${row.tool}` || busy === `disconnect:${row.tool}` || busy === `switch:${row.tool}`;
  const status = statusText(row, name, t);
  const hasDetails = row.status !== "unsupported";
  // Connected, but some ZCode providers added since do not go through the relay.
  const warn = row.unrouted > 0 && (row.status === "active" || row.status === "paused");

  return (
    <li data-testid={`rewrite-agent-${row.tool}`} data-state={row.status}>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 px-4 py-3 sm:px-5">
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-muted/70 ring-1 ring-border">
          <ToolLogo tool={row.tool} size={20} />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-[14px] font-medium text-foreground">{name}</p>
          <p className={cx("flex items-center gap-1.5 text-[12.5px]", warn ? TEXT.bypassed : TEXT[row.status])} data-testid={`rewrite-status-${row.tool}`}>
            <span aria-hidden="true" className={cx("inline-block h-2 w-2 shrink-0 rounded-full", warn ? DOT.bypassed : DOT[row.status])} />
            <span className="min-w-0">{status}</span>
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {row.connected && row.status !== "bypassed" ? (
            <Switch
              size="sm"
              aria-label={t("rewrite.agents.toggle", { agent: name })}
              checked={row.enabled}
              disabled={!masterOn || (busy !== null && !rowBusy)}
              busy={busy === `switch:${row.tool}`}
              data-testid={`rewrite-toggle-agent-${row.tool}`}
              onCheckedChange={onToggle}
            />
          ) : null}
          <AgentAction row={row} busy={busy} rowBusy={rowBusy} onConnect={onConnect} onDisconnect={onDisconnect} />
          {hasDetails ? (
            <button
              type="button"
              aria-expanded={open}
              aria-controls={detailsId}
              aria-label={t("rewrite.action.details")}
              title={t("rewrite.action.details")}
              data-testid={`rewrite-details-${row.tool}`}
              onClick={() => setOpen((value) => !value)}
              className="flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <IconChevronDown size={14} className={cx("transition-transform duration-200", open && "rotate-180")} />
            </button>
          ) : (
            <span className="w-8" aria-hidden="true" />
          )}
        </div>
      </div>
      {open && hasDetails ? (
        <dl
          id={detailsId}
          className="mx-4 mb-3 grid gap-1 rounded-xl bg-muted/50 px-3 py-2.5 text-[12.5px] text-muted-foreground sm:mx-5"
          data-testid={`rewrite-details-body-${row.tool}`}
        >
          {row.file ? (
            <dd>
              {t("rewrite.details.writes", {
                file: row.file,
                field: row.field ?? t("rewrite.details.providerField"),
              })}
            </dd>
          ) : null}
          {row.hosts.length === 1 ? <dd>{t("rewrite.details.upstream", { host: row.hosts[0] })}</dd> : null}
          {row.hosts.length > 1 ? <dd>{t("rewrite.details.upstreams", { hosts: row.hosts.join(", ") })}</dd> : null}
          {row.oauthNote ? <dd>{t("rewrite.details.oauth")}</dd> : null}
        </dl>
      ) : null}
    </li>
  );
}

function statusText(row: AgentRow, agent: string, t: (key: string, options?: Record<string, unknown>) => string) {
  switch (row.status) {
    case "active":
      return row.unrouted > 0 ? t("rewrite.status.unrouted", { count: row.unrouted }) : t("rewrite.status.active");
    case "paused":
      return t("rewrite.status.paused");
    case "unlinked":
      return row.unsupported ? t(`rewrite.unsupported.${row.unsupported}`) : t("rewrite.status.unlinked");
    case "bypassed":
      return `${t("rewrite.status.bypassed")} · ${t(`rewrite.bypassed.${row.bypassed}`)}`;
    case "down":
      return t("rewrite.status.notRunning", { agent });
    case "unsupported":
      return `${t("rewrite.status.unsupported")} · ${t(`rewrite.unsupported.${row.unsupported}`)}`;
  }
}

function AgentAction({
  row,
  busy,
  rowBusy,
  onConnect,
  onDisconnect,
}: {
  row: AgentRow;
  busy: string | null;
  rowBusy: boolean;
  onConnect: () => void;
  onDisconnect: () => void;
}) {
  const { t } = useTranslation();
  const locked = busy !== null && !rowBusy;
  if (row.status === "unsupported") return null;
  if (row.status === "unlinked") {
    return (
      <Button
        size="sm"
        variant="outline"
        data-testid={`rewrite-connect-${row.tool}`}
        loading={busy === `connect:${row.tool}`}
        disabled={locked || !row.canConnect}
        onClick={onConnect}
      >
        {t("rewrite.action.connect")}
      </Button>
    );
  }
  const repair = row.status === "bypassed" || row.status === "down" || row.unrouted > 0;
  return (
    <>
      {repair ? (
        <Button
          size="sm"
          variant="primary"
          data-testid={`rewrite-reconnect-${row.tool}`}
          loading={busy === `connect:${row.tool}`}
          disabled={locked}
          onClick={onConnect}
        >
          {row.status === "down" ? t("rewrite.action.restart") : t("rewrite.action.reconnect")}
        </Button>
      ) : null}
      <Button
        size="sm"
        variant="danger"
        data-testid={`rewrite-disconnect-${row.tool}`}
        disabled={locked}
        onClick={onDisconnect}
      >
        {t("rewrite.action.disconnect")}
      </Button>
    </>
  );
}
