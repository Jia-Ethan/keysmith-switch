import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import type { ToastApi } from "../hooks/useToasts";
import { baseName, formatBytes } from "../lib/format";
import { loadHarnessStatus } from "../lib/harnessState";
import { readableError } from "../lib/planFailure";
import type { PlanFailure } from "../lib/planFailure";
import type { CleanupCategory, ExtraMeta, RollbackPlan, SnapshotMeta } from "../types";
import { ConfirmDialog } from "./ConfirmDialog";
import { EmptyState } from "./EmptyState";
import { Callout, PlanFailureNotice } from "./PlanPreview";
import { IconAlert, IconCheck, IconHistory } from "./icons";
import { ToolLogo } from "./ToolLogos";
import { Button } from "./ui";

function categoryOf(extra: ExtraMeta): CleanupCategory {
  if (extra.category) return extra.category;
  return extra.kind === "erased" ? "login-and-cache" : "setup";
}

function summarizeExtras(
  extras: ExtraMeta[] | undefined,
  absentCount: number,
  t: ReturnType<typeof useTranslation>["t"],
): string[] {
  const saved = (extras ?? []).filter((e) => e.kind !== "erased");
  const parts: string[] = [];
  const sessions = saved.filter((e) => categoryOf(e) === "sessions");
  const setup = saved.filter((e) => {
    const cat = categoryOf(e);
    return cat === "setup" || cat === "memory" || cat === "memories" || cat === "login-and-cache";
  });
  const deployExtras = saved.filter((e) => categoryOf(e) === "deployment");

  if (sessions.length > 0) {
    const bytes = sessions.reduce((sum, item) => sum + item.bytes, 0);
    parts.push(t("versions.sessionsSummary", { count: sessions.length, size: formatBytes(bytes) }));
  }
  if (setup.length > 0) {
    const bytes = setup.reduce((sum, item) => sum + item.bytes, 0);
    parts.push(t("versions.setupSummary", { count: setup.length, size: formatBytes(bytes) }));
  }
  if (deployExtras.length > 0) {
    parts.push(t("versions.deploymentExtrasSummary", { count: deployExtras.length }));
  }
  if (absentCount > 0) {
    parts.push(t("versions.absentSummary", { count: absentCount }));
  }
  return parts;
}

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
          {snapshots.map((snapshot) => {
            const extraParts = summarizeExtras(snapshot.extras, snapshot.absentExtras?.length ?? 0, t);
            return (
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
                    {[
                      describe(snapshot, t),
                      snapshot.memory && snapshot.memory.bytes > 0
                        ? t("versions.memory", { file: baseName(snapshot.memory.path), size: formatBytes(snapshot.memory.bytes) })
                        : null,
                      snapshot.memories && snapshot.memories.files > 0
                        ? t("versions.memories", { files: snapshot.memories.files, size: formatBytes(snapshot.memories.bytes) })
                        : null,
                      ...extraParts,
                    ]
                      .filter(Boolean)
                      .join(" · ")}
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
            );
          })}
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
  const reqSeq = useRef(0);

  const load = useCallback(async () => {
    const seq = ++reqSeq.current;
    if (!snapshot) {
      setPlan(null);
      setFailure(null);
      setLoading(false);
      return;
    }
    setLoading(true);
    setFailure(null);
    setPlan(null);
    try {
      const next = await api.planRollback(snapshot.id);
      if (seq !== reqSeq.current) return;
      setPlan(next);
    } catch (reason) {
      if (seq !== reqSeq.current) return;
      setFailure({ message: readableError(reason, t), detail: null });
    } finally {
      if (seq === reqSeq.current) setLoading(false);
    }
  }, [snapshot, t]);

  useEffect(() => {
    void load();
    return () => {
      reqSeq.current += 1;
    };
  }, [load]);

  const activePlan = plan && snapshot && plan.snapshot.id === snapshot.id ? plan : null;

  const confirm = async () => {
    if (!activePlan || !snapshot || working) return;
    setWorking(true);
    setFailure(null);
    try {
      const done = await api.confirmRollback(activePlan.operationId);
      await onDone(snapshot, Boolean(done.savedSnapshotId));
      onClose();
    } catch (reason) {
      setFailure({ message: readableError(reason, t), detail: null });
    } finally {
      setWorking(false);
    }
  };

  const restoresMemories = Boolean(activePlan?.memories && activePlan.memories.restoreFiles > 0);
  const restoresExtras = (activePlan?.snapshot.extras ?? []).filter((e) => e.kind !== "erased");
  const absentExtras = activePlan?.snapshot.absentExtras ?? [];
  const sessionExtras = restoresExtras.filter((e) => categoryOf(e) === "sessions");
  const otherExtras = restoresExtras.filter((e) => categoryOf(e) !== "sessions");
  const restoresSessions = sessionExtras.length > 0;
  // Backend computes `agentRunning` authoritatively (including legacy snapshots & absentExtras).
  const needsAgentStopped = Boolean(activePlan?.agentRunning);
  const restoresSomething = Boolean(
    activePlan &&
      (activePlan.snapshot.deployment.restorable ||
        (activePlan.memory && activePlan.memory.restoreBytes > 0) ||
        restoresMemories ||
        restoresExtras.length > 0 ||
        absentExtras.length > 0),
  );
  const memoryFile = activePlan?.snapshot.memory ? baseName(activePlan.snapshot.memory.path) : "";
  const title = activePlan?.snapshot.deployment.title ? `「${activePlan.snapshot.deployment.title}」` : "";
  const currentTitle = activePlan?.currentTitle ? `「${activePlan.currentTitle}」` : "";
  const toolName = snapshot ? t(`nav.${snapshot.tool}`) : "";
  const sessionFiles = sessionExtras.reduce((sum, e) => sum + e.files, 0);
  const sessionBytes = sessionExtras.reduce((sum, e) => sum + e.bytes, 0);
  const hasStrippedConfig = restoresExtras.some((e) => e.kind === "saved-without-login");

  return (
    <ConfirmDialog
      open={snapshot !== null}
      wide
      icon={snapshot ? <ToolLogo tool={snapshot.tool} size={22} /> : undefined}
      title={t("versions.rollbackTitle")}
      description={snapshot ? toolName : undefined}
      confirmLabel={t("versions.confirmRollback")}
      cancelLabel={t("common.cancel")}
      closeLabel={t("common.close")}
      busy={working}
      confirmDisabled={
        loading ||
        !activePlan ||
        activePlan.blockers.length > 0 ||
        !restoresSomething ||
        Boolean(failure) ||
        needsAgentStopped
      }
      confirmTestId="rollback-confirm"
      onClose={() => {
        if (!working) onClose();
      }}
      onConfirm={() => void confirm()}
    >
      {loading ? <p className="text-[13.5px] text-muted-foreground">{t("versions.checking")}</p> : null}
      {activePlan ? (
        <div className="space-y-3" data-testid="rollback-plan">
          {restoresSomething ? (
            <ul className="space-y-2 text-[13.5px] text-foreground">
              {activePlan.snapshot.deployment.restorable ? (
                <li className="flex items-start gap-2" data-testid="rollback-deploy">
                  <Tick />
                  <span>
                    {t("versions.willDeploy", { title: title || t("versions.untitled") })}
                    {activePlan.replacesDeployment ? (
                      <span className="mt-0.5 block text-[12.5px] text-muted-foreground">
                        {t("versions.willReplace", { title: currentTitle })}
                      </span>
                    ) : null}
                  </span>
                </li>
              ) : null}
              {activePlan.memory && activePlan.memory.restoreBytes > 0 ? (
                <li className="flex items-start gap-2" data-testid="rollback-memory">
                  <Tick />
                  <span>
                    {t("versions.willRestoreMemory", { file: memoryFile, size: formatBytes(activePlan.memory.restoreBytes) })}
                    {activePlan.memory.currentDiffers && activePlan.memory.currentBytes > 0 ? (
                      <span className="mt-0.5 block text-[12.5px] text-warning">
                        {t("versions.currentMemory", { file: memoryFile, size: formatBytes(activePlan.memory.currentBytes) })}
                      </span>
                    ) : null}
                  </span>
                </li>
              ) : null}
              {activePlan.memories && restoresMemories ? (
                <li className="flex items-start gap-2" data-testid="rollback-memories">
                  <Tick />
                  <span>
                    {t("versions.willRestoreMemories", { files: activePlan.memories.restoreFiles, size: formatBytes(activePlan.memories.restoreBytes) })}
                    {activePlan.memories.currentFiles > 0 ? (
                      <span className="mt-0.5 block text-[12.5px] text-warning">
                        {t("versions.currentMemories", { files: activePlan.memories.currentFiles })}
                      </span>
                    ) : null}
                  </span>
                </li>
              ) : null}
              {restoresSessions ? (
                <li className="flex items-start gap-2" data-testid="rollback-sessions">
                  <Tick />
                  <span>
                    {t("versions.willRestoreSessions", {
                      count: sessionExtras.length,
                      files: sessionFiles,
                      size: formatBytes(sessionBytes),
                    })}
                    <span className="mt-0.5 block text-[12px] text-muted-foreground">
                      {sessionExtras.map((e) => e.name).join(" · ")}
                    </span>
                  </span>
                </li>
              ) : null}
              {otherExtras.map((extra) => (
                <li className="flex items-start gap-2" key={extra.name} data-testid="rollback-extra">
                  <Tick />
                  <span>
                    {t(extra.kind === "saved-without-login" ? "versions.willRestoreExtraWithoutLogin" : "versions.willRestoreExtra", {
                      name: extra.name,
                    })}
                  </span>
                </li>
              ))}
              {absentExtras.map((item) => (
                <li className="flex items-start gap-2" key={`absent:${item.name}`} data-testid="rollback-absent">
                  <Tick />
                  <span>{t("versions.willMoveAsideAbsent", { name: item.name })}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-[13.5px] text-muted-foreground">{t("versions.nothingToRestore")}</p>
          )}
          {needsAgentStopped ? (
            <Callout tone="warn" icon={<IconAlert size={14} />}>
              <span data-testid="rollback-memories-running">{t("versions.memoriesRunning", { tool: toolName })}</span>
            </Callout>
          ) : null}
          {hasStrippedConfig || restoresSessions ? (
            <p className="text-[12.5px] text-warning" data-testid="rollback-no-login">
              {t("versions.noLoginBack")}
            </p>
          ) : null}
          {!activePlan.snapshot.deployment.restorable && activePlan.snapshot.deployment.present ? (
            <p className="text-[12.5px] text-warning">{t("versions.cannotRedeploy")}</p>
          ) : null}
          {activePlan.savesCurrent ? <p className="text-[12.5px] text-muted-foreground">{t("versions.savesCurrent")}</p> : null}
          {activePlan.blockers.length > 0 ? (
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
