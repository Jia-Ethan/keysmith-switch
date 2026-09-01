import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { FirstRunReport, ImportCandidate } from "../types";
import { ConfirmDialog } from "./ConfirmDialog";
import { Checkbox, Mono } from "./ui";

export function FirstRunDialog({
  open,
  candidates,
  sidecar,
  onImport,
  onSkip,
}: {
  open: boolean;
  candidates: ImportCandidate[];
  sidecar: FirstRunReport["sidecar"] | null;
  onImport: (paths: string[]) => void;
  onSkip: () => void;
}) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const paths = candidates.filter((item) => selected[item.id]).map((item) => item.path);
  const hasCandidates = candidates.length > 0;

  return (
    <ConfirmDialog
      open={open}
      title={t(hasCandidates ? "firstRun.title" : "firstRun.checkTitle")}
      description={t(hasCandidates ? "firstRun.hint" : "firstRun.checkHint")}
      confirmLabel={hasCandidates ? t("firstRun.import") : t("firstRun.complete")}
      cancelLabel={hasCandidates ? t("firstRun.skip") : t("common.cancel")}
      closeLabel={t("common.close")}
      confirmDisabled={hasCandidates && paths.length === 0}
      onClose={onSkip}
      onConfirm={() => (hasCandidates ? onImport(paths) : onSkip())}
    >
      <div className="flex flex-col gap-3">
        {sidecar ? (
          <div className="rounded-2xl border border-border">
            <p className="border-b border-border px-3 py-2 font-medium">{t("firstRun.environment")}</p>
            <ul className="divide-y divide-border">
              {sidecar.tools.map((item) => (
                <li key={item.tool} className="flex items-center justify-between gap-3 px-3 py-2">
                  <span>{t(`nav.${item.tool}`)}</span>
                  <span className={item.available ? "text-primary" : "text-destructive"}>
                    {item.available ? t("firstRun.ready") : t("status.unavailable")}
                  </span>
                </li>
              ))}
            </ul>
            <p className="border-t border-border px-3 py-2 text-muted-foreground">
              {sidecar.pythonRequired ? t("firstRun.pythonRequired") : t("firstRun.noPythonRequired")}
            </p>
          </div>
        ) : null}
        {hasCandidates ? (
          candidates.map((item) => (
            <Checkbox
              key={item.id}
              label={
                <span>
                  {item.title} · {item.tool}
                  <Mono className="ml-2">{item.path}</Mono>
                </span>
              }
              checked={Boolean(selected[item.id])}
              onChange={(event) =>
                setSelected((current) => ({ ...current, [item.id]: event.target.checked }))
              }
            />
          ))
        ) : (
          <p className="text-muted-foreground">{t("firstRun.noCandidates")}</p>
        )}
      </div>
    </ConfirmDialog>
  );
}
