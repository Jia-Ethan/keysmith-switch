import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { gatePlan } from "../lib/planGate";
import { shortPath } from "../lib/format";
import type { Envelope } from "../types";
import { Disclosure, Mono, SectionLabel } from "./ui";
import { IconAlert } from "./icons";

export function PlanPreview({ envelope }: { envelope: Envelope }) {
  const { t } = useTranslation();
  const gate = gatePlan(envelope);
  const drifted =
    envelope.status === "drift" ||
    envelope.status === "recovery-required" ||
    envelope.recoveryRequired;

  return (
    <div className="space-y-3 text-[13px]" data-testid="plan-preview">
      {drifted ? (
        <div className="flex items-start gap-2 rounded-lg border border-amber-600/40 bg-amber-500/10 px-3 py-2 text-amber-700 dark:text-amber-400">
          <IconAlert size={14} className="mt-px shrink-0" />
          <p className="min-w-0">{t("plan.driftNoOverwrite")}</p>
        </div>
      ) : null}

      {!gate.ok ? (
        <div className="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-destructive">
          <IconAlert size={14} className="mt-px shrink-0" />
          <div className="min-w-0">
            <p className="font-medium">{t("plan.blocked")}</p>
            <ul className="mt-1 list-inside list-disc space-y-0.5">
              {gate.reasons.map((reason, index) => (
                <li key={index}>{reason}</li>
              ))}
            </ul>
          </div>
        </div>
      ) : null}

      <Section title={t("plan.files")}>
        {envelope.plannedFiles.length === 0 ? (
          <p className="text-muted-foreground">{t("common.none")}</p>
        ) : (
          <ul className="space-y-1">
            {envelope.plannedFiles.map((file, index) => (
              <li key={index} className="break-all">
                <span className="font-semibold text-primary">{file.action}</span>{" "}
                <Mono>{shortPath(file.path)}</Mono>
                {file.detail ? <p className="ml-4 text-muted-foreground">{file.detail}</p> : null}
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title={t("plan.backups")}>
        {envelope.backups.length === 0 ? (
          <p className="text-muted-foreground">{t("common.none")}</p>
        ) : (
          <ul className="space-y-1">
            {envelope.backups.map((item, index) => (
              <li key={index} className="break-all">
                <Mono>{shortPath(item.target)}</Mono>
                {item.backupPath ? (
                  <>
                    {" → "}
                    <Mono>{shortPath(item.backupPath)}</Mono>
                  </>
                ) : null}
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

      <div className="grid grid-cols-2 gap-x-3 gap-y-1">
        <div>
          <span className="text-muted-foreground">{t("plan.fingerprintCurrent")}</span>
        </div>
        <div>
          <Mono>{envelope.currentFingerprint ?? "—"}</Mono>
        </div>
        <div>
          <span className="text-muted-foreground">{t("plan.fingerprintTarget")}</span>
        </div>
        <div>
          <Mono>{envelope.targetFingerprint ?? "—"}</Mono>
        </div>
      </div>

      {envelope.warnings.length > 0 ? (
        <Section title={t("plan.warnings")}>
          <ul className="list-inside list-disc space-y-0.5 text-muted-foreground">
            {envelope.warnings.map((item, index) => (
              <li key={index}>{item}</li>
            ))}
          </ul>
        </Section>
      ) : null}

      {envelope.reloadHint ? (
        <p className="text-muted-foreground">
          {t("plan.reloadHint")}: {envelope.reloadHint}
        </p>
      ) : null}

      {envelope.error ? (
        <p className="rounded-lg bg-destructive/10 px-2 py-1.5 text-destructive">{envelope.error}</p>
      ) : null}

      <Disclosure title={t("common.details")} testId="plan-advanced">
        <div className="space-y-2 text-[11px]">
          <div>
            <span className="text-muted-foreground">CLI:</span>{" "}
            <Mono>{envelope.cliPath ?? "—"}</Mono>
          </div>
          <div>
            <span className="text-muted-foreground">argv:</span>{" "}
            <Mono>{envelope.argv.join(" ") || "—"}</Mono>
          </div>
          <div>
            <span className="text-muted-foreground">exit:</span> {envelope.exitCode}
          </div>
          {envelope.redactedStderr ? (
            <div>
              <span className="text-muted-foreground">stderr:</span>
              <pre className="mt-1 overflow-auto whitespace-pre-wrap rounded bg-muted px-2 py-1">
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
    <section className="space-y-1">
      <SectionLabel>{title}</SectionLabel>
      {children}
    </section>
  );
}
