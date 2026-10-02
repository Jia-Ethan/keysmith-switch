import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import type { ToastApi } from "../hooks/useToasts";
import { baseName, formatBytes } from "../lib/format";
import { loadHarnessStatus } from "../lib/harnessState";
import { readableError } from "../lib/planFailure";
import type { PlanFailure } from "../lib/planFailure";
import type { RollbackPlan, SnapshotMeta } from "../types";
import { ConfirmDialog } from "./ConfirmDialog";
import { EmptyState } from "./EmptyState";
import { Callout, PlanFailureNotice } from "./PlanPreview";
import { IconAlert, IconCheck, IconHistory } from "./icons";
import { ToolLogo } from "./ToolLogos";
import { Button } from "./ui";

/** The saved versions: made before a cleanup, or before a rollback overwrote something. */
export function VersionsTab({ toast, onChanged }: { toast: ToastApi; onChanged?: () => void }) {
  const { t, i18n } = useTranslation();
  const [snapshots, setSnapshots] = useState<SnapshotMeta[] | null>(null);
  const [loadError, setLoadError] = useState(false);
  const [rolling, setRolling] = useState<SnapshotMeta | null>(null);
  const [deleting, setDeleting] = useState<SnapshotMeta | null>(null);

  const load = useCallback(async () => {
    try {
      setSnapshots(await api.listSnapshots());
      setLoadError(false);
    } catch {
      setLoadError(true);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const when = (iso: string) =>
    new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(new Date(iso));

  const remove = async (snapshot: SnapshotMeta) => {
    setDeleting(null);
    try {
      await api.deleteSnapshot(snapshot.id);
      toast.ok(t("versions.deleted"));
      await load();
    } catch (reason) {
      toast.err(readableError(reason, t));
    }
  };

  return (
    <div className="space-y-4" data-testid="versions-tab">
      <div>
        <h2 className="text-[16px] font-semibold text-foreground">{t("versions.title")}</h2>
        <p className="mt-0.5 text-[13px] text-muted-foreground">{t("versions.lead")}</p>
      </div>

      {loadError ? (
        <Callout tone="danger" icon={<IconAlert size={14} />}>
          {t("errors.loadFailed")}
        </Callout>
      ) : null}

      {snapshots && snapshots.length === 0 ? (
        <EmptyState icon={<IconHistory size={22} />} title={t("versions.empty")} hint={t("versions.emptyHint")} testId="versions-empty" />
      ) : null}

      {snapshots && snapshots.length > 0 ? (
        <ul className="space-y-2" data-testid="versions-list">
          {snapshots.map((snapshot) => (
            <li key={snapshot.id} className="surface-card flex flex-wrap items-center gap-3 p-3.5" data-testid={`version-${snapshot.id}`}>
              <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-card ring-1 ring-border">
                <ToolLogo tool={snapshot.tool} size={22} />
              </span>
              <div className="min-w-0 flex-1">
                <p className="flex flex-wrap items-center gap-2 text-[14px] font-medium text-foreground">
                  {t(`nav.${snapshot.tool}`)}
                  <span className="rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                    {snapshot.kind === "cleanup" ? t("versions.kindCleanup") : t("versions.kindBeforeRollback")}
                  </span>
                </p>
                <p className="mt-0.5 text-[12.5px] text-muted-foreground">{when(snapshot.createdAt)}</p>
                <p className="mt-0.5 truncate text-[12.5px] text-muted-foreground">
                  {describe(snapshot, t)}
                  {snapshot.memory && snapshot.memory.bytes > 0
                    ? ` · ${t("versions.memory", { file: baseName(snapshot.memory.path), size: formatBytes(snapshot.memory.bytes) })}`
                    : ""}
                  {snapshot.memories && snapshot.memories.files > 0
                    ? ` · ${t("versions.memories", { files: snapshot.memories.files, size: formatBytes(snapshot.memories.bytes) })}`
                    : ""}
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <Button size="sm" variant="ghost" data-testid={`version-delete-${snapshot.id}`} onClick={() => setDeleting(snapshot)}>
                  {t("versions.delete")}
                </Button>
                <Button size="sm" variant="primary" data-testid={`version-rollback-${snapshot.id}`} onClick={() => setRolling(snapshot)}>
                  {t("versions.rollback")}
                </Button>
              </div>
            </li>
          ))}
        </ul>
      ) : null}

      <RollbackDialog
        snapshot={rolling}
        onClose={() => setRolling(null)}
        onDone={async (snapshot, saved) => {
          // The agent runs a different prompt now: the pages that show it must read it again.
          void loadHarnessStatus(snapshot.tool, true);
          toast.ok(saved ? t("versions.rolledBackSaved") : t("versions.rolledBack"));
          onChanged?.();
          await load();
        }}
      />

      <ConfirmDialog
        open={deleting !== null}
        danger
        title={t("versions.deleteTitle")}
        description={deleting ? `${t(`nav.${deleting.tool}`)} · ${when(deleting.createdAt)}` : undefined}
        confirmLabel={t("versions.delete")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        confirmTestId="version-confirm-delete"
        onClose={() => setDeleting(null)}
        onConfirm={() => deleting && void remove(deleting)}
      >
        <p className="text-[13.5px] text-muted-foreground">{t("versions.deleteBody")}</p>
      </ConfirmDialog>
    </div>
  );
}

function describe(snapshot: SnapshotMeta, t: ReturnType<typeof useTranslation>["t"]): string {
  const { deployment } = snapshot;
  if (!deployment.present) return t("versions.noDeployment");
  if (!deployment.restorable) return t("versions.deployedLost");
  return deployment.title ? t("versions.deployed", { title: deployment.title }) : t("versions.deployedUnknown");
}

function RollbackDialog({
  snapshot,
  onClose,
  onDone,
}: {
  snapshot: SnapshotMeta | null;
  onClose: () => void;
  onDone: (snapshot: SnapshotMeta, savedCurrent: boolean) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [plan, setPlan] = useState<RollbackPlan | null>(null);
  const [loading, setLoading] = useState(false);
  const [working, setWorking] = useState(false);
  const [failure, setFailure] = useState<PlanFailure | null>(null);

  const load = useCallback(async () => {
    if (!snapshot) return;
    setLoading(true);
    setFailure(null);
    setPlan(null);
    try {
      setPlan(await api.planRollback(snapshot.id));
    } catch (reason) {
      setFailure({ message: readableError(reason, t), detail: null });
    } finally {
      setLoading(false);
    }
  }, [snapshot, t]);

  useEffect(() => {
    void load();
  }, [load]);

  const confirm = async () => {
    if (!plan || !snapshot || working) return;
    setWorking(true);
    setFailure(null);
    try {
      const done = await api.confirmRollback(plan.operationId);
      await onDone(snapshot, Boolean(done.savedSnapshotId));
      onClose();
    } catch (reason) {
      setFailure({ message: readableError(reason, t), detail: null });
    } finally {
      setWorking(false);
    }
  };

  const restoresMemories = Boolean(plan?.memories && plan.memories.restoreFiles > 0);
  const restoresExtras = (plan?.snapshot.extras ?? []).filter((e) => e.kind !== "erased");
  const restoresSomething = Boolean(
    plan &&
      (plan.snapshot.deployment.restorable ||
        (plan.memory && plan.memory.restoreBytes > 0) ||
        restoresMemories ||
        restoresExtras.length > 0),
  );
  const memoryFile = plan?.snapshot.memory ? baseName(plan.snapshot.memory.path) : "";
  const title = plan?.snapshot.deployment.title ? `「${plan.snapshot.deployment.title}」` : "";
  const currentTitle = plan?.currentTitle ? `「${plan.currentTitle}」` : "";

  return (
    <ConfirmDialog
      open={snapshot !== null}
      wide
      icon={snapshot ? <ToolLogo tool={snapshot.tool} size={22} /> : undefined}
      title={t("versions.rollbackTitle")}
      description={snapshot ? t(`nav.${snapshot.tool}`) : undefined}
      confirmLabel={t("versions.confirmRollback")}
      cancelLabel={t("common.cancel")}
      closeLabel={t("common.close")}
      busy={working}
      confirmDisabled={
        loading || !plan || plan.blockers.length > 0 || !restoresSomething || Boolean(failure) || (restoresMemories && plan?.agentRunning === true)
      }
      confirmTestId="rollback-confirm"
      onClose={() => {
        if (!working) onClose();
      }}
      onConfirm={() => void confirm()}
    >
      {loading ? <p className="text-[13.5px] text-muted-foreground">{t("versions.checking")}</p> : null}
      {plan ? (
        <div className="space-y-3" data-testid="rollback-plan">
          {restoresSomething ? (
            <ul className="space-y-2 text-[13.5px] text-foreground">
              {plan.snapshot.deployment.restorable ? (
                <li className="flex items-start gap-2" data-testid="rollback-deploy">
                  <Tick />
                  <span>
                    {t("versions.willDeploy", { title: title || t("versions.untitled") })}
                    {plan.replacesDeployment ? (
                      <span className="mt-0.5 block text-[12.5px] text-muted-foreground">
                        {t("versions.willReplace", { title: currentTitle })}
                      </span>
                    ) : null}
                  </span>
                </li>
              ) : null}
              {plan.memory && plan.memory.restoreBytes > 0 ? (
                <li className="flex items-start gap-2" data-testid="rollback-memory">
                  <Tick />
                  <span>
                    {t("versions.willRestoreMemory", { file: memoryFile, size: formatBytes(plan.memory.restoreBytes) })}
                    {plan.memory.currentDiffers && plan.memory.currentBytes > 0 ? (
                      <span className="mt-0.5 block text-[12.5px] text-warning">
                        {t("versions.currentMemory", { file: memoryFile, size: formatBytes(plan.memory.currentBytes) })}
                      </span>
                    ) : null}
                  </span>
                </li>
              ) : null}
              {plan.memories && restoresMemories ? (
                <li className="flex items-start gap-2" data-testid="rollback-memories">
                  <Tick />
                  <span>
                    {t("versions.willRestoreMemories", { files: plan.memories.restoreFiles, size: formatBytes(plan.memories.restoreBytes) })}
                    {plan.memories.currentFiles > 0 ? (
                      <span className="mt-0.5 block text-[12.5px] text-warning">
                        {t("versions.currentMemories", { files: plan.memories.currentFiles })}
                      </span>
                    ) : null}
                    {plan.agentRunning ? (
                      <span className="mt-0.5 block text-[12.5px] text-warning" data-testid="rollback-memories-running">
                        {t("versions.memoriesRunning")}
                      </span>
                    ) : null}
                  </span>
                </li>
              ) : null}
              {restoresExtras.map((extra) => (
                <li className="flex items-start gap-2" key={extra.name} data-testid="rollback-extra">
                  <Tick />
                  <span>
                    {t(extra.kind === "saved-without-login" ? "versions.willRestoreExtraWithoutLogin" : "versions.willRestoreExtra", {
                      name: extra.name,
                    })}
                  </span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-[13.5px] text-muted-foreground">{t("versions.nothingToRestore")}</p>
          )}
          {restoresExtras.length > 0 ? (
            <p className="text-[12.5px] text-warning" data-testid="rollback-no-login">
              {t("versions.noLoginBack")}
            </p>
          ) : null}
          {!plan.snapshot.deployment.restorable && plan.snapshot.deployment.present ? (
            <p className="text-[12.5px] text-warning">{t("versions.cannotRedeploy")}</p>
          ) : null}
          {plan.savesCurrent ? <p className="text-[12.5px] text-muted-foreground">{t("versions.savesCurrent")}</p> : null}
          {plan.blockers.length > 0 ? (
            <Callout tone="danger" icon={<IconAlert size={14} />}>
              {t("cleanup.repair")}
            </Callout>
          ) : null}
        </div>
      ) : null}
      {failure ? <PlanFailureNotice failure={failure} busy={working || loading} onReplan={() => void load()} /> : null}
    </ConfirmDialog>
  );
}

function Tick() {
  return (
    <span className="mt-[3px] flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-success/15 text-success">
      <IconCheck size={11} />
    </span>
  );
}
