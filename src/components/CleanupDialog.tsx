import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { readableError } from "../lib/planFailure";
import type { PlanFailure } from "../lib/planFailure";
import { formatBytes } from "../lib/format";
import type { CleanupPlan, CleanupResult, ToolId } from "../types";
import { ConfirmDialog } from "./ConfirmDialog";
import { Callout, PlanFailureNotice } from "./PlanPreview";
import { IconAlert, IconCheck, IconShield } from "./icons";
import { ToolLogo } from "./ToolLogos";
import { Button, Checkbox } from "./ui";

/**
 * Clean an agent back to nothing: say plainly what goes, what stays, and that a version is
 * saved first. The one thing that cannot be undone by Keysmith alone, emptying what the
 * person wrote in CLAUDE.md, needs a tick before the button works.
 */
export function CleanupDialog({
  tool,
  toolName,
  open,
  onClose,
  onDone,
  onOpenVersions,
}: {
  tool: ToolId;
  toolName: string;
  open: boolean;
  onClose: () => void;
  onDone: (result: CleanupResult) => void;
  onOpenVersions: () => void;
}) {
  const { t } = useTranslation();
  const [plan, setPlan] = useState<CleanupPlan | null>(null);
  const [loading, setLoading] = useState(false);
  const [working, setWorking] = useState(false);
  const [ack, setAck] = useState(false);
  const [failure, setFailure] = useState<PlanFailure | null>(null);
  const [result, setResult] = useState<CleanupResult | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setFailure(null);
    setPlan(null);
    setAck(false);
    try {
      setPlan(await api.planCleanup(tool));
    } catch (reason) {
      setFailure({ message: readableError(reason, t), detail: null });
    } finally {
      setLoading(false);
    }
  }, [tool, t]);

  useEffect(() => {
    if (!open) return;
    setResult(null);
    void load();
  }, [open, load]);

  const memory = plan?.memory && plan.memory.bytes > 0 ? plan.memory : null;
  const needsAck = Boolean(memory);
  const blocked = Boolean(plan && plan.blockers.length > 0);
  const confirm = async () => {
    if (!plan || working) return;
    setWorking(true);
    setFailure(null);
    try {
      const done = await api.confirmCleanup(plan.operationId);
      setResult(done);
      onDone(done);
    } catch (reason) {
      // A plan that was confirmed once is spent, whatever went wrong.
      setFailure({ message: readableError(reason, t), detail: null });
    } finally {
      setWorking(false);
    }
  };

  const deploymentTitle = plan?.deployment.title ? `「${plan.deployment.title}」` : "";

  return (
    <ConfirmDialog
      open={open}
      wide
      danger
      icon={<ToolLogo tool={tool} size={22} />}
      title={t("cleanup.title", { tool: toolName })}
      confirmLabel={result ? t("common.close") : t("cleanup.confirm")}
      cancelLabel={result ? t("common.close") : t("common.cancel")}
      closeLabel={t("common.close")}
      busy={working}
      confirmDisabled={
        result
          ? false
          : loading || !plan || plan.nothingToDo || blocked || Boolean(failure) || (needsAck && !ack)
      }
      confirmTestId="cleanup-confirm"
      onClose={onClose}
      onConfirm={() => (result ? onClose() : void confirm())}
    >
      {loading ? <p className="text-[13.5px] text-muted-foreground" data-testid="cleanup-loading">{t("cleanup.checking")}</p> : null}

      {plan && !result ? (
        <div className="space-y-3" data-testid="cleanup-plan">
          {plan.nothingToDo ? (
            <p className="text-[13.5px] text-muted-foreground" data-testid="cleanup-nothing">{t("cleanup.nothing")}</p>
          ) : (
            <ul className="space-y-2 text-[13.5px] text-foreground">
              {plan.deployment.present ? (
                <li className="flex items-start gap-2" data-testid="cleanup-deployment">
                  <span className="mt-[3px] flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-primary/15 text-primary">
                    <IconCheck size={11} />
                  </span>
                  <span className="min-w-0">
                    {t("cleanup.willRemoveDeployment", { title: deploymentTitle })}
                    {!plan.deployment.restorable ? (
                      <span className="mt-0.5 block text-[12.5px] text-warning">{t("cleanup.unsaved")}</span>
                    ) : null}
                  </span>
                </li>
              ) : null}
              {memory ? (
                <li className="flex items-start gap-2" data-testid="cleanup-memory">
                  <span className="mt-[3px] flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-destructive/15 text-destructive">
                    <IconAlert size={11} />
                  </span>
                  <span className="min-w-0 font-medium">
                    {t("cleanup.willEmptyMemory", { lines: memory.lines, size: formatBytes(memory.bytes) })}
                  </span>
                </li>
              ) : null}
            </ul>
          )}

          {plan.blockers.length > 0 ? (
            <Callout tone="danger" icon={<IconAlert size={14} />}>
              <span data-testid="cleanup-blocked">{t("cleanup.repair")}</span>
            </Callout>
          ) : null}

          {!plan.nothingToDo ? (
            <>
              <p className="flex items-start gap-2 rounded-xl bg-muted/60 px-3 py-2 text-[12.5px] leading-relaxed text-muted-foreground">
                <IconShield size={14} className="mt-0.5 shrink-0 text-success" />
                <span>
                  {t("cleanup.saves")}
                  <br />
                  {t("cleanup.untouched")}
                </span>
              </p>
              {needsAck ? (
                <Checkbox
                  data-testid="cleanup-ack"
                  checked={ack}
                  onChange={(event) => setAck(event.target.checked)}
                  label={t("cleanup.ack")}
                />
              ) : null}
            </>
          ) : null}
        </div>
      ) : null}

      {result ? (
        <div className="space-y-3" data-testid="cleanup-done">
          <p className="flex items-center gap-2 text-[13.5px] font-medium text-success">
            <IconCheck size={14} />
            {result.snapshotId ? t("cleanup.done") : t("cleanup.doneNothing")}
          </p>
          {result.snapshotId ? (
            <Button size="sm" variant="outline" data-testid="cleanup-open-versions" onClick={onOpenVersions}>
              {t("cleanup.openVersions")}
            </Button>
          ) : null}
        </div>
      ) : null}

      {failure ? <PlanFailureNotice failure={failure} busy={working || loading} onReplan={() => void load()} /> : null}
    </ConfirmDialog>
  );
}
