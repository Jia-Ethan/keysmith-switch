import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { gatePlan } from "../lib/planGate";
import { CONFIG_DRIFT, isNoise, localizeReason, needsCleanup, needsReconcile } from "../lib/planFailure";
import type { PlanFailure } from "../lib/planFailure";
import { shortPath } from "../lib/format";
import type { Envelope } from "../types";
import { Button, Disclosure, Mono, cx } from "./ui";
import { IconAlert, IconArrowRight, IconCheck, IconFile, IconInfo, IconShield } from "./icons";
import { ToolLogo } from "./ToolLogos";
import type { ToolId } from "../types";

type ActionTone = "add" | "change" | "remove" | "neutral";

const ACTION_TONES: Record<string, ActionTone> = {
  create: "add",
  add: "add",
  write: "change",
  modify: "change",
  update: "change",
  replace: "change",
  patch: "change",
  remove: "remove",
  delete: "remove",
  restore: "change",
};

const TONE_CLASS: Record<ActionTone, string> = {
  add: "bg-success/10 text-success ring-success/25",
  change: "bg-primary/10 text-primary ring-primary/25",
  remove: "bg-destructive/10 text-destructive ring-destructive/25",
  neutral: "bg-muted text-muted-foreground ring-border",
};

function splitPath(path: string): { dir: string; name: string } {
  const short = shortPath(path);
  const index = Math.max(short.lastIndexOf("/"), short.lastIndexOf("\\"));
  if (index < 0) return { dir: "", name: short };
  return { dir: short.slice(0, index + 1), name: short.slice(index + 1) };
}

function PathLabel({ path, className }: { path: string; className?: string }) {
  const { dir, name } = splitPath(path);
  return (
    <span className={cx("min-w-0 break-all font-mono text-[12px] leading-snug", className)} title={path}>
      <span className="text-muted-foreground">{dir}</span>
      <span className="font-semibold text-foreground">{name}</span>
    </span>
  );
}

function shortFingerprint(value: string | null): string {
  if (!value) return "—";
  const [algo, hash] = value.includes(":") ? value.split(":", 2) : ["", value];
  const trimmed = hash.length > 12 ? `${hash.slice(0, 6)}…${hash.slice(-4)}` : hash;
  return algo ? `${algo}:${trimmed}` : trimmed;
}

export interface PlanPreviewProps {
  envelope: Envelope;
  /** With the agent and the titles involved, the preview reads as a sentence instead of a report. */
  tool?: ToolId;
  kind?: "activate" | "deactivate" | "recover" | "reconcile";
  /** What the agent runs now, when known. */
  fromTitle?: string | null;
  /** What it will run after the plan. */
  toTitle?: string | null;
}

/**
 * A plan, said plainly first: what changes, that it is backed up, what to do
 * when it cannot go ahead. Everything technical (files, backups, fingerprints)
 * is one click away under "details" and never needed to decide.
 */
export function PlanPreview({ envelope, tool, kind = "activate", fromTitle, toTitle }: PlanPreviewProps) {
  const { t } = useTranslation();
  const gate = gatePlan(envelope);
  const drifted =
    envelope.status === "drift" ||
    envelope.status === "recovery-required" ||
    envelope.recoveryRequired;
  const toolName = tool ? t(`nav.${tool}`) : "";
  const ready = gate.ok && !drifted;
  // What a person can act on: the bookkeeping reasons go, and the adapter's drift wording
  // becomes a sentence. The raw reasons stay in "details".
  const meaningful = gate.reasons.filter((reason) => !isNoise(reason));
  const reasons = (meaningful.length > 0 ? meaningful : gate.reasons).map((reason) =>
    reason.includes(CONFIG_DRIFT) ? t("plan.blockerConfigDrift", { tool: toolName || t("plan.theAgent") }) : localizeReason(reason, t),
  );
  const warnings = envelope.warnings.filter((item) => !envelope.blockers.includes(item));

  return (
    <div className="space-y-4" data-testid="plan-preview">
      {tool && (kind === "activate" || kind === "deactivate") ? (
        <PlanFlow tool={tool} kind={kind} fromTitle={fromTitle} toTitle={toTitle} ready={ready} />
      ) : null}

      {ready ? (
        <ul className="space-y-1.5 text-[13.5px] text-foreground" data-testid="plan-friendly">
          <FriendlyLine>
            {kind === "reconcile"
              ? t("plan.friendlyReconcile", { tool: toolName })
              : kind === "deactivate"
                ? t("plan.friendlyOff", { tool: toolName })
                : fromTitle
                  ? t("plan.friendlyReplace", { tool: toolName })
                  : t("plan.friendlyFresh", { tool: toolName })}
          </FriendlyLine>
          {envelope.backups.length > 0 && kind !== "reconcile" ? <FriendlyLine>{t("plan.friendlySafe")}</FriendlyLine> : null}
          {envelope.reloadRequired && kind !== "deactivate" && kind !== "reconcile" ? (
            <FriendlyLine>{t("plan.friendlyReload", { tool: toolName })}</FriendlyLine>
          ) : null}
        </ul>
      ) : null}

      {drifted ? (
        <Callout tone="warn" icon={<IconAlert size={14} />}>
          {t("plan.driftNoOverwrite")}
        </Callout>
      ) : null}

      {!gate.ok ? (
        <Callout tone="danger" icon={<IconAlert size={14} />}>
          <p className="font-medium">{t("plan.blocked")}</p>
          <ul className="mt-1 list-inside list-disc space-y-0.5">
            {reasons.map((reason, index) => (
              <li key={index}>{reason}</li>
            ))}
          </ul>
          {tool && needsReconcile(tool, envelope) ? <p className="mt-1.5 font-medium">{t("plan.hintReconcile")}</p> : null}
          {tool && needsCleanup(tool, envelope) ? <p className="mt-1.5 font-medium">{t("plan.hintCleanup")}</p> : null}
        </Callout>
      ) : null}

      {envelope.conflicts.length > 0 ? (
        <Callout tone="warn" icon={<IconAlert size={14} />}>
          <ul className="space-y-0.5">
            {envelope.conflicts.map((item, index) => {
              const path = typeof item === "string" ? item : item.path;
              const reason = typeof item === "string" ? item : item.reason;
              return (
                <li key={index} className="break-all">
                  {shortPath(path)}
                  {reason && reason !== path ? ` — ${reason}` : ""}
                </li>
              );
            })}
          </ul>
        </Callout>
      ) : null}

      {warnings.length > 0 ? (
        <Callout tone="warn" icon={<IconAlert size={14} />}>
          <ul className="list-inside list-disc space-y-0.5">
            {warnings.map((item, index) => (
              <li key={index}>{localizeReason(item, t)}</li>
            ))}
          </ul>
        </Callout>
      ) : null}

      {envelope.reloadHint ? (
        <Callout tone="info" icon={<IconInfo size={14} />}>
          {envelope.reloadHint}
        </Callout>
      ) : null}

      {envelope.error ? (
        <Callout tone="danger" icon={<IconAlert size={14} />}>
          {localizeReason(envelope.error, t)}
        </Callout>
      ) : null}

      <Disclosure title={t("plan.details")} testId="plan-details">
        <PlanDetails envelope={envelope} />
      </Disclosure>
    </div>
  );
}

function FriendlyLine({ children }: { children: ReactNode }) {
  return (
    <li className="flex items-start gap-2">
      <span className="mt-[3px] flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-success/15 text-success">
        <IconCheck size={11} />
      </span>
      <span className="min-w-0">{children}</span>
    </li>
  );
}

/** Before → after, with the agent in the middle and a current of light between the two. */
function PlanFlow({
  tool,
  kind,
  fromTitle,
  toTitle,
  ready,
}: {
  tool: ToolId;
  kind: "activate" | "deactivate";
  fromTitle?: string | null;
  toTitle?: string | null;
  ready: boolean;
}) {
  const { t } = useTranslation();
  const off = kind === "deactivate";
  return (
    <div
      className="plan-flow relative flex items-center gap-3 overflow-hidden rounded-2xl border border-border bg-muted/40 px-4 py-4"
      data-ready={ready || undefined}
      data-testid="plan-flow"
    >
      <FlowCard label={t("plan.flowNow")} title={fromTitle || t("plan.flowNothing")} dim={!fromTitle} />
      <div className="relative flex shrink-0 flex-col items-center gap-1">
        <span className="plan-flow-orb flex h-11 w-11 items-center justify-center rounded-2xl bg-card shadow-glow ring-1 ring-primary/25">
          <ToolLogo tool={tool} size={24} />
        </span>
        <IconArrowRight size={14} className="plan-flow-arrow text-primary" />
      </div>
      <FlowCard
        label={off ? t("plan.flowAfter") : t("plan.flowNext")}
        title={off ? t("plan.flowNothing") : toTitle || "—"}
        dim={off}
        accent={!off}
      />
    </div>
  );
}

function FlowCard({ label, title, dim, accent }: { label: string; title: string; dim?: boolean; accent?: boolean }) {
  return (
    <div
      className={cx(
        "min-w-0 flex-1 rounded-xl border px-3 py-2.5",
        accent ? "border-primary/35 bg-primary/[0.07]" : "border-border bg-card",
      )}
    >
      <p className="text-[11px] font-medium text-muted-foreground">{label}</p>
      <p className={cx("mt-0.5 truncate text-[14px] font-semibold", dim ? "text-muted-foreground" : "text-foreground")} title={title}>
        {title}
      </p>
    </div>
  );
}

function PlanDetails({ envelope }: { envelope: Envelope }) {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <Section title={t("plan.files")}>
        {envelope.plannedFiles.length === 0 ? (
          <p className="text-muted-foreground">{t("common.none")}</p>
        ) : (
          <ul className="divide-y divide-border overflow-hidden rounded-xl border border-border bg-background/50">
            {envelope.plannedFiles.map((file, index) => {
              const tone = ACTION_TONES[file.action.toLowerCase()] ?? "neutral";
              return (
                <li key={index} className="flex items-start gap-2.5 px-3 py-2.5">
                  <IconFile size={15} className="mt-0.5 shrink-0 text-muted-foreground" />
                  <div className="min-w-0 flex-1">
                    <PathLabel path={file.path} />
                    {file.detail ? (
                      <p className="mt-0.5 text-[12px] text-muted-foreground">{file.detail}</p>
                    ) : null}
                  </div>
                  <span
                    className={cx(
                      "shrink-0 rounded-md px-1.5 py-0.5 text-[11px] font-semibold uppercase tracking-wide ring-1 ring-inset",
                      TONE_CLASS[tone],
                    )}
                  >
                    {file.action}
                  </span>
                </li>
              );
            })}
          </ul>
        )}
      </Section>

      <Section title={t("plan.backups")}>
        {envelope.backups.length === 0 ? (
          <p className="text-muted-foreground">{t("common.none")}</p>
        ) : (
          <ul className="space-y-1.5">
            {envelope.backups.map((item, index) => (
              <li
                key={index}
                className="flex items-start gap-2.5 rounded-xl border border-dashed border-border px-3 py-2"
              >
                <IconShield size={15} className="mt-0.5 shrink-0 text-success" />
                <div className="min-w-0 flex-1 space-y-0.5">
                  <PathLabel path={item.target} />
                  {item.backupPath ? (
                    <div className="flex items-start gap-1 text-muted-foreground">
                      <IconArrowRight size={12} className="mt-[3px] shrink-0" />
                      <Mono className="text-[11.5px]">{shortPath(item.backupPath)}</Mono>
                    </div>
                  ) : null}
                </div>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title={t("plan.fingerprint")}>
        <div className="flex flex-wrap items-center gap-2 font-mono text-[12px]" data-testid="plan-fingerprints">
          <span
            className="rounded-lg bg-muted px-2 py-1 text-muted-foreground"
            title={`${t("plan.fingerprintCurrent")}: ${envelope.currentFingerprint ?? "—"}`}
          >
            {shortFingerprint(envelope.currentFingerprint)}
          </span>
          <IconArrowRight size={13} className="text-muted-foreground" />
          <span
            className="rounded-lg bg-primary/10 px-2 py-1 text-primary"
            title={`${t("plan.fingerprintTarget")}: ${envelope.targetFingerprint ?? "—"}`}
          >
            {shortFingerprint(envelope.targetFingerprint)}
          </span>
        </div>
      </Section>

      <div className="space-y-2 text-[12.5px]" data-testid="plan-advanced">
        <div>
          <span className="text-muted-foreground">CLI:</span> <Mono>{envelope.cliPath ?? "—"}</Mono>
        </div>
        <div>
          <span className="text-muted-foreground">argv:</span> <Mono>{envelope.argv.join(" ") || "—"}</Mono>
        </div>
        <div>
          <span className="text-muted-foreground">exit:</span> {envelope.exitCode}
        </div>
        {envelope.redactedStderr ? (
          <div>
            <span className="text-muted-foreground">stderr:</span>
            <pre className="mt-1 overflow-auto whitespace-pre-wrap rounded-lg bg-muted px-2.5 py-2 font-mono leading-snug">
              {envelope.redactedStderr}
            </pre>
          </div>
        ) : null}
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="space-y-1.5">
      <h3 className="text-[11.5px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">{title}</h3>
      {children}
    </section>
  );
}

const CALLOUT_TONE = {
  warn: "border-warning/35 bg-warning/10 text-warning",
  danger: "border-destructive/35 bg-destructive/[0.08] text-destructive",
  info: "border-border bg-muted/60 text-muted-foreground",
} as const;

export function Callout({
  tone,
  icon,
  children,
}: {
  tone: keyof typeof CALLOUT_TONE;
  icon?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className={cx("flex items-start gap-2 rounded-xl border px-3 py-2 text-[12.5px] leading-relaxed", CALLOUT_TONE[tone])}>
      {icon ? <span className="mt-[3px] shrink-0">{icon}</span> : null}
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}

/**
 * A confirmed plan that did not go through. The plan is spent either way, so the way
 * forward is a new plan, not another click on the same one.
 */
export function PlanFailureNotice({
  failure,
  onReplan,
  busy,
}: {
  failure: PlanFailure;
  onReplan: () => void;
  busy: boolean;
}) {
  const { t } = useTranslation();
  return (
    <div className="mt-3 space-y-2" data-testid="plan-failure">
      <Callout tone="danger" icon={<IconAlert size={14} />}>
        <p className="font-medium">{t("plan.failed")}</p>
        <p className="mt-0.5">{failure.message}</p>
        {failure.detail ? (
          <details className="mt-1.5">
            <summary className="cursor-pointer text-[12px] opacity-80">{t("plan.details")}</summary>
            <pre className="mt-1 max-h-32 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-background/60 px-2.5 py-1.5 font-mono text-[11.5px] leading-snug">
              {failure.detail}
            </pre>
          </details>
        ) : null}
      </Callout>
      <Button size="sm" variant="outline" disabled={busy} data-testid="plan-replan" onClick={onReplan}>
        {t("plan.replan")}
      </Button>
    </div>
  );
}
