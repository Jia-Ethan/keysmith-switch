import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { gatePlan } from "../lib/planGate";
import { shortPath } from "../lib/format";
import type { Envelope } from "../types";

export function PlanPreview({ envelope }: { envelope: Envelope }) {
  const { t } = useTranslation();
  const gate = gatePlan(envelope);
  const drifted =
    envelope.status === "drift" ||
    envelope.status === "recovery-required" ||
    envelope.recoveryRequired;

  return (
    <div className="space-y-3" data-testid="plan-preview">
      {drifted ? (
        <p className="rounded border border-amber-800/50 bg-amber-950/40 px-2 py-1.5 text-amber-100">
          {t("plan.driftNoOverwrite")}
        </p>
      ) : null}
      {!gate.ok ? (
        <div className="rounded border border-rose-800/50 bg-rose-950/40 px-2 py-1.5 text-rose-100">
          <p className="font-medium">{t("plan.blocked")}</p>
          <ul className="mt-1 list-disc pl-4">
            {gate.reasons.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
        </div>
      ) : null}

      <Section title={t("plan.files")}>
        {envelope.plannedFiles.length === 0 ? (
          <p>{t("common.none")}</p>
        ) : (
          envelope.plannedFiles.map((file) => (
            <div key={`${file.action}:${file.path}`} className="font-mono">
              <span className="text-accent-600">{file.action}</span> {shortPath(file.path)}
              {file.detail ? <span className="block text-ink-200">{file.detail}</span> : null}
            </div>
          ))
        )}
      </Section>

      <Section title={t("plan.backups")}>
        {envelope.backups.length === 0 ? (
          <p>{t("common.none")}</p>
        ) : (
          envelope.backups.map((item) => (
            <div key={item.target} className="font-mono">
              {shortPath(item.target)}
              {item.backupPath ? ` → ${shortPath(item.backupPath)}` : ""}
            </div>
          ))
        )}
      </Section>

      <Section title={t("plan.conflicts")}>
        {envelope.conflicts.length === 0 ? (
          <p>{t("common.none")}</p>
        ) : (
          envelope.conflicts.map((item) => {
            const path = typeof item === "string" ? item : item.path;
            const reason = typeof item === "string" ? item : item.reason;
            return (
              <div key={path} className="text-rose-200">
                {shortPath(path)}
                {reason && reason !== path ? ` — ${reason}` : ""}
              </div>
            );
          })
        )}
      </Section>

      <div className="grid grid-cols-2 gap-2 font-mono">
        <div>
          <div className="text-ink-200">{t("plan.fingerprintCurrent")}</div>
          <div>{envelope.currentFingerprint ?? "—"}</div>
        </div>
        <div>
          <div className="text-ink-200">{t("plan.fingerprintTarget")}</div>
          <div>{envelope.targetFingerprint ?? "—"}</div>
        </div>
      </div>

      {envelope.warnings.length > 0 ? (
        <Section title={t("plan.warnings")}>
          {envelope.warnings.map((item) => (
            <div key={item}>{item}</div>
          ))}
        </Section>
      ) : null}

      {envelope.reloadHint ? (
        <p>
          {t("plan.reloadHint")}: {envelope.reloadHint}
        </p>
      ) : null}

      {envelope.error ? <p className="text-rose-200">{envelope.error}</p> : null}
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-ink-200">{title}</h3>
      <div className="space-y-1">{children}</div>
    </section>
  );
}
