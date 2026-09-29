import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { gatePlan } from "../lib/planGate";
import { shortPath } from "../lib/format";
import type { Envelope } from "../types";
import { Disclosure, Mono, cx } from "./ui";
import { IconAlert, IconArrowRight, IconFile, IconInfo, IconShield } from "./icons";

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

export function PlanPreview({ envelope }: { envelope: Envelope }) {
  const { t } = useTranslation();
  const gate = gatePlan(envelope);
  const drifted =
    envelope.status === "drift" ||
    envelope.status === "recovery-required" ||
    envelope.recoveryRequired;

  return (
    <div className="space-y-4" data-testid="plan-preview">
      <div className="flex flex-wrap items-center gap-1.5 text-[12px]">
        <SummaryChip>{t("plan.summaryFiles", { count: envelope.plannedFiles.length })}</SummaryChip>
        <SummaryChip>{t("plan.summaryBackups", { count: envelope.backups.length })}</SummaryChip>
        {gate.ok && !drifted ? (
          <SummaryChip tone="ok">
            <IconShield size={12} />
            {t("plan.reviewed")}
          </SummaryChip>
        ) : null}
      </div>

      {drifted ? (
        <Callout tone="warn" icon={<IconAlert size={14} />}>
          {t("plan.driftNoOverwrite")}
        </Callout>
      ) : null}

      {!gate.ok ? (
        <Callout tone="danger" icon={<IconAlert size={14} />}>
          <p className="font-medium">{t("plan.blocked")}</p>
          <ul className="mt-1 list-inside list-disc space-y-0.5">
            {gate.reasons.map((reason, index) => (
              <li key={index}>{reason}</li>
            ))}
          </ul>
        </Callout>
      ) : null}

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

      {envelope.conflicts.length > 0 ? (
        <Section title={t("plan.conflicts")}>
          <ul className="space-y-1">
            {envelope.conflicts.map((item, index) => {
              const path = typeof item === "string" ? item : item.path;
              const reason = typeof item === "string" ? item : item.reason;
              return (
                <li key={index} className="break-all text-destructive">
                  <Mono className="text-destructive">{shortPath(path)}</Mono>
                  {reason && reason !== path ? ` — ${reason}` : ""}
                </li>
              );
            })}
          </ul>
        </Section>
      ) : null}

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

      {envelope.warnings.length > 0 ? (
        <Callout tone="warn" icon={<IconAlert size={14} />}>
          <p className="font-medium">{t("plan.warnings")}</p>
          <ul className="mt-0.5 list-inside list-disc space-y-0.5">
            {envelope.warnings.map((item, index) => (
              <li key={index}>{item}</li>
            ))}
          </ul>
        </Callout>
      ) : null}

      {envelope.reloadHint ? (
        <Callout tone="info" icon={<IconInfo size={14} />}>
          {t("plan.reloadHint")}: {envelope.reloadHint}
        </Callout>
      ) : null}

      {envelope.error ? (
        <Callout tone="danger" icon={<IconAlert size={14} />}>
          {envelope.error}
        </Callout>
      ) : null}

      <Disclosure title={t("common.details")} testId="plan-advanced">
        <div className="space-y-2 text-[12.5px]">
          <div>
            <span className="text-muted-foreground">CLI:</span> <Mono>{envelope.cliPath ?? "—"}</Mono>
          </div>
          <div>
            <span className="text-muted-foreground">argv:</span> <Mono>{envelope.argv.join(" ") || "—"}</Mono>
          </div>
          <div>
            <span className="text-muted-foreground">{t("plan.fingerprintCurrent")}:</span>{" "}
            <Mono>{envelope.currentFingerprint ?? "—"}</Mono>
          </div>
          <div>
            <span className="text-muted-foreground">{t("plan.fingerprintTarget")}:</span>{" "}
            <Mono>{envelope.targetFingerprint ?? "—"}</Mono>
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
      </Disclosure>
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

function SummaryChip({ children, tone }: { children: ReactNode; tone?: "ok" }) {
  return (
    <span
      className={cx(
        "inline-flex items-center gap-1 rounded-full px-2 py-0.5 font-medium",
        tone === "ok" ? "bg-success/10 text-success" : "bg-muted text-muted-foreground",
      )}
    >
      {children}
    </span>
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
