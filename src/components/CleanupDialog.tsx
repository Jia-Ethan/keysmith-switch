import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { baseName, formatBytes } from "../lib/format";
import { readableError } from "../lib/planFailure";
import type { PlanFailure } from "../lib/planFailure";
import type { CleanupCategory, CleanupPlan, CleanupResult, CleanupSelection, ExtraMeta, ToolId } from "../types";
import { ConfirmDialog } from "./ConfirmDialog";
import { Callout, PlanFailureNotice } from "./PlanPreview";
import { IconAlert, IconCheck, IconChevronDown, IconChevronRight, IconShield } from "./icons";
import { ToolLogo } from "./ToolLogos";
import { Button, Checkbox, cx } from "./ui";

const EMPTY_SELECTION: CleanupSelection = {
  deployment: false,
  memory: false,
  memories: false,
  setup: false,
  sessions: false,
  loginAndCache: false,
};

function categoryOf(extra: ExtraMeta): CleanupCategory {
  if (extra.category) return extra.category;
  return extra.kind === "erased" ? "login-and-cache" : "setup";
}

function sumExtras(items: ExtraMeta[]): { files: number; bytes: number } {
  return items.reduce(
    (acc, item) => ({ files: acc.files + item.files, bytes: acc.bytes + item.bytes }),
    { files: 0, bytes: 0 },
  );
}

/**
 * Scoped, opt-in agent cleanup:
 * - Defaults to Keysmith's own deployment and leftovers only, leaving sessions, memories, skills,
 *   rules and logins untouched unless explicitly ticked.
 * - Separates recoverable items (saved into a local version before removal, including session
 *   history) from irreversible deletions (logins, plugins, caches), and gates irreversible
 *   deletions behind an explicit confirmation checkbox.
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
  const [selection, setSelection] = useState<CleanupSelection>(EMPTY_SELECTION);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [ackMemory, setAckMemory] = useState(false);
  const [ackErase, setAckErase] = useState(false);
  const [failure, setFailure] = useState<PlanFailure | null>(null);
  const [result, setResult] = useState<CleanupResult | null>(null);
  const reqSeq = useRef(0);

  const load = useCallback(async () => {
    const seq = ++reqSeq.current;
    setLoading(true);
    setFailure(null);
    setPlan(null);
    setSelection(EMPTY_SELECTION);
    setExpanded({});
    setAckMemory(false);
    setAckErase(false);
    try {
      const next = await api.planCleanup(tool);
      if (seq !== reqSeq.current) return;
      setPlan(next);
      const hasDeployment =
        next.deployment.present || (next.extras ?? []).some((e) => categoryOf(e) === "deployment");
      // Safe default: only Keysmith's own deployment/leftovers are pre-selected.
      setSelection({
        deployment: hasDeployment,
        memory: false,
        memories: false,
        setup: false,
        sessions: false,
        loginAndCache: false,
      });
    } catch (reason) {
      if (seq !== reqSeq.current) return;
      setFailure({ message: readableError(reason, t), detail: null });
    } finally {
      if (seq === reqSeq.current) setLoading(false);
    }
  }, [tool, t]);

  useEffect(() => {
    if (!open) {
      reqSeq.current += 1;
      return;
    }
    setResult(null);
    void load();
    return () => {
      reqSeq.current += 1;
    };
  }, [open, load]);

  const activePlan = plan && plan.tool === tool ? plan : null;

  const groups = useMemo(() => {
    const extras = activePlan?.extras ?? [];
    const deploymentExtras = extras.filter((e) => categoryOf(e) === "deployment");
    const memoryExtras = extras.filter((e) => categoryOf(e) === "memory");
    const memoriesExtras = extras.filter((e) => categoryOf(e) === "memories");
    const setupExtras = extras.filter((e) => categoryOf(e) === "setup");
    const sessionExtras = extras.filter((e) => categoryOf(e) === "sessions");
    const loginCacheExtras = extras.filter((e) => categoryOf(e) === "login-and-cache");
    const memoryFile = activePlan?.memory && activePlan.memory.bytes > 0 ? activePlan.memory : null;
    const memoriesFolder = activePlan?.memories && activePlan.memories.files > 0 ? activePlan.memories : null;

    return {
      deploymentExtras,
      memoryExtras,
      memoriesExtras,
      setupExtras,
      sessionExtras,
      loginCacheExtras,
      memoryFile,
      memoriesFolder,
      hasDeployment: Boolean(activePlan?.deployment.present || deploymentExtras.length > 0),
      hasMemory: Boolean(memoryFile || memoryExtras.length > 0),
      hasMemories: Boolean(memoriesFolder || memoriesExtras.length > 0),
      hasSetup: setupExtras.length > 0,
      hasSessions: sessionExtras.length > 0,
      hasLoginAndCache: Boolean(loginCacheExtras.length > 0 || activePlan?.loginInKeychain),
    };
  }, [activePlan]);

  const effectiveSelection: CleanupSelection = {
    deployment: selection.deployment && groups.hasDeployment,
    memory: selection.memory && groups.hasMemory,
    memories: selection.memories && groups.hasMemories,
    setup: selection.setup && groups.hasSetup,
    sessions: selection.sessions && groups.hasSessions,
    loginAndCache: selection.loginAndCache && groups.hasLoginAndCache,
  };

  const anySelected = Object.values(effectiveSelection).some(Boolean);
  const needsMemoryAck = Boolean(effectiveSelection.memory && groups.memoryFile);
  const needsEraseAck = effectiveSelection.loginAndCache;
  // Only categories that touch live agent folders, SQLite DBs, or credentials require quitting first.
  const needsAgentStopped = Boolean(
    activePlan?.agentRunning &&
      (effectiveSelection.memories || effectiveSelection.sessions || effectiveSelection.loginAndCache),
  );
  const blocked = Boolean(activePlan && activePlan.blockers.length > 0);
  const hasRecoverable =
    groups.hasDeployment || groups.hasMemory || groups.hasMemories || groups.hasSetup || groups.hasSessions;

  const memoryFileName = groups.memoryFile ? baseName(groups.memoryFile.path) : "";
  const keptItems = useMemo(() => {
    const items: string[] = [];
    if (groups.hasSessions && !effectiveSelection.sessions) items.push(t("cleanup.keptSessions"));
    if (groups.hasSetup && !effectiveSelection.setup) items.push(t("cleanup.keptSetup"));
    if (groups.hasMemory && !effectiveSelection.memory) {
      items.push(memoryFileName ? t("cleanup.keptMemory", { file: memoryFileName }) : t("cleanup.catMemoryGenericTitle"));
    }
    if (groups.hasMemories && !effectiveSelection.memories) items.push(t("cleanup.keptMemories"));
    if (groups.hasLoginAndCache && !effectiveSelection.loginAndCache) items.push(t("cleanup.keptLogin"));
    if (groups.hasDeployment && !effectiveSelection.deployment) items.push(t("cleanup.keptDeployment"));
    return items;
  }, [effectiveSelection, groups, memoryFileName, t]);

  const toggle = (key: keyof CleanupSelection, checked: boolean) => {
    setSelection((prev) => ({ ...prev, [key]: checked }));
    if (key === "memory" && !checked) setAckMemory(false);
    if (key === "loginAndCache" && !checked) setAckErase(false);
  };

  const toggleDetails = (key: string) => {
    setExpanded((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  const confirm = async () => {
    if (!activePlan || working || !anySelected) return;
    setWorking(true);
    setFailure(null);
    try {
      const done = await api.confirmCleanup(activePlan.operationId, effectiveSelection);
      setResult(done);
      onDone(done);
    } catch (reason) {
      // A plan that was confirmed once is spent, whatever went wrong.
      setFailure({ message: readableError(reason, t), detail: null });
    } finally {
      setWorking(false);
    }
  };

  const deploymentTitle = activePlan?.deployment.title ? `「${activePlan.deployment.title}」` : "";
  const cleanedAnything = Boolean(
    result &&
      (result.snapshotId ||
        result.deactivated ||
        result.memoryCleared ||
        result.memoriesCleared ||
        (result.extrasCleared ?? 0) > 0 ||
        (result.erased ?? 0) > 0 ||
        result.keychainCleared),
  );
  const doneHeadline = result
    ? result.snapshotId
      ? t("cleanup.done")
      : cleanedAnything
        ? t("cleanup.doneErasedOnly")
        : t("cleanup.doneNothing")
    : "";

  const setupStats = sumExtras(groups.setupExtras);
  const sessionStats = sumExtras(groups.sessionExtras);
  const loginStats = sumExtras(groups.loginCacheExtras);
  const memoriesExtraStats = sumExtras(groups.memoriesExtras);
  const memoriesTotalFiles = (groups.memoriesFolder?.files ?? 0) + memoriesExtraStats.files;
  const memoriesTotalBytes = (groups.memoriesFolder?.bytes ?? 0) + memoriesExtraStats.bytes;

  return (
    <ConfirmDialog
      open={open}
      wide
      danger={needsEraseAck}
      icon={<ToolLogo tool={tool} size={22} />}
      title={t("cleanup.title", { tool: toolName })}
      confirmLabel={
        result
          ? t("common.close")
          : needsEraseAck
            ? t("cleanup.confirmIrreversible")
            : t("cleanup.confirm")
      }
      cancelLabel={result ? t("common.close") : t("common.cancel")}
      closeLabel={t("common.close")}
      busy={working}
      confirmDisabled={
        result
          ? false
          : loading ||
            !activePlan ||
            activePlan.nothingToDo ||
            !anySelected ||
            blocked ||
            needsAgentStopped ||
            Boolean(failure) ||
            (needsMemoryAck && !ackMemory) ||
            (needsEraseAck && !ackErase)
      }
      confirmTestId="cleanup-confirm"
      onClose={onClose}
      onConfirm={() => (result ? onClose() : void confirm())}
    >
      {loading ? (
        <p className="text-[13.5px] text-muted-foreground" data-testid="cleanup-loading">
          {t("cleanup.checking")}
        </p>
      ) : null}

      {activePlan && !result ? (
        <div className="space-y-3.5" data-testid="cleanup-plan">
          {activePlan.nothingToDo ? (
            <p className="text-[13.5px] text-muted-foreground" data-testid="cleanup-nothing">
              {t("cleanup.nothing")}
            </p>
          ) : (
            <>
              <p className="text-[12.5px] leading-relaxed text-muted-foreground" data-testid="cleanup-reassurance">
                {t("cleanup.reassurance")}
              </p>

              <div
                className="rounded-xl border border-success/25 bg-success/[0.06] px-3 py-2 text-[12.5px] leading-relaxed"
                data-testid="cleanup-kept-summary"
              >
                <div className="flex items-start gap-2">
                  <IconShield size={14} className="mt-0.5 shrink-0 text-success-strong" />
                  <div className="min-w-0 space-y-1">
                    <p className="font-medium text-success-strong">{t("cleanup.keptTitle")}</p>
                    <div className="flex flex-wrap gap-1.5">
                      {keptItems.map((label) => (
                        <span
                          key={label}
                          className="rounded-full bg-card px-2 py-0.5 text-[11.5px] font-medium text-foreground ring-1 ring-success/25"
                        >
                          {label}
                        </span>
                      ))}
                      <span className="rounded-full bg-card/70 px-2 py-0.5 text-[11.5px] text-muted-foreground ring-1 ring-border">
                        {t("cleanup.keptAlways")}
                      </span>
                    </div>
                  </div>
                </div>
              </div>

              {hasRecoverable ? (
                <section className="space-y-2" data-testid="cleanup-recoverable-section">
                  <h3 className="text-[12px] font-semibold tracking-[0.01em] text-muted-foreground">
                    {t("cleanup.sectionRecoverable")}
                  </h3>

                  {groups.hasDeployment ? (
                    <OptionCard
                      testId="cleanup-deployment"
                      checkboxTestId="cleanup-opt-deployment"
                      checked={effectiveSelection.deployment}
                      onChange={(checked) => toggle("deployment", checked)}
                      title={t("cleanup.catDeploymentTitle")}
                      badge={t("cleanup.badgeDefault")}
                      secondaryBadge={t("cleanup.badgeRecoverable")}
                      description={
                        activePlan.deployment.present
                          ? t("cleanup.willRemoveDeployment", { title: deploymentTitle })
                          : t("cleanup.catDeploymentDesc")
                      }
                      warning={
                        activePlan.deployment.present && !activePlan.deployment.restorable
                          ? t("cleanup.unsaved")
                          : undefined
                      }
                      details={groups.deploymentExtras}
                      detailsKey="deployment"
                      expanded={Boolean(expanded.deployment)}
                      onToggleDetails={() => toggleDetails("deployment")}
                    />
                  ) : null}

                  {groups.hasMemory ? (
                    <OptionCard
                      testId="cleanup-memory"
                      checkboxTestId="cleanup-opt-memory"
                      checked={effectiveSelection.memory}
                      onChange={(checked) => toggle("memory", checked)}
                      title={
                        memoryFileName
                          ? t("cleanup.catMemoryTitle", { file: memoryFileName })
                          : t("cleanup.catMemoryGenericTitle")
                      }
                      secondaryBadge={t("cleanup.badgeRecoverable")}
                      meta={
                        groups.memoryFile
                          ? t("cleanup.summaryMemory", {
                              lines: groups.memoryFile.lines,
                              size: formatBytes(groups.memoryFile.bytes),
                            })
                          : undefined
                      }
                      description={
                        groups.memoryFile
                          ? t("cleanup.willEmptyMemory", {
                              file: memoryFileName,
                              lines: groups.memoryFile.lines,
                              size: formatBytes(groups.memoryFile.bytes),
                            })
                          : t("cleanup.catMemoryDesc")
                      }
                      details={groups.memoryExtras}
                      detailsKey="memory"
                      expanded={Boolean(expanded.memory)}
                      onToggleDetails={() => toggleDetails("memory")}
                    />
                  ) : null}

                  {groups.hasMemories ? (
                    <OptionCard
                      testId="cleanup-memories"
                      checkboxTestId="cleanup-opt-memories"
                      checked={effectiveSelection.memories}
                      onChange={(checked) => toggle("memories", checked)}
                      title={t("cleanup.catMemoriesTitle")}
                      secondaryBadge={t("cleanup.badgeRecoverable")}
                      runningBadge={activePlan.agentRunning ? t("cleanup.badgeNeedsQuit", { tool: toolName }) : undefined}
                      meta={t("cleanup.summaryFiles", {
                        files: memoriesTotalFiles,
                        size: formatBytes(memoriesTotalBytes),
                      })}
                      description={t("cleanup.catMemoriesDesc")}
                      details={groups.memoriesExtras}
                      detailsKey="memories"
                      expanded={Boolean(expanded.memories)}
                      onToggleDetails={() => toggleDetails("memories")}
                    />
                  ) : null}

                  {groups.hasSetup ? (
                    <OptionCard
                      testId="cleanup-setup"
                      checkboxTestId="cleanup-opt-setup"
                      checked={effectiveSelection.setup}
                      onChange={(checked) => toggle("setup", checked)}
                      title={t("cleanup.catSetupTitle")}
                      secondaryBadge={t("cleanup.badgeRecoverable")}
                      meta={t("cleanup.summaryItems", {
                        count: groups.setupExtras.length,
                        files: setupStats.files,
                        size: formatBytes(setupStats.bytes),
                      })}
                      description={t("cleanup.catSetupDesc")}
                      details={groups.setupExtras}
                      detailsKey="setup"
                      expanded={Boolean(expanded.setup)}
                      onToggleDetails={() => toggleDetails("setup")}
                    />
                  ) : null}

                  {groups.hasSessions ? (
                    <OptionCard
                      testId="cleanup-sessions"
                      checkboxTestId="cleanup-opt-sessions"
                      checked={effectiveSelection.sessions}
                      onChange={(checked) => toggle("sessions", checked)}
                      title={t("cleanup.catSessionsTitle")}
                      secondaryBadge={t("cleanup.badgeRecoverable")}
                      runningBadge={activePlan.agentRunning ? t("cleanup.badgeNeedsQuit", { tool: toolName }) : undefined}
                      meta={t("cleanup.summaryItems", {
                        count: groups.sessionExtras.length,
                        files: sessionStats.files,
                        size: formatBytes(sessionStats.bytes),
                      })}
                      description={t("cleanup.catSessionsDesc")}
                      details={groups.sessionExtras}
                      detailsKey="sessions"
                      expanded={Boolean(expanded.sessions)}
                      onToggleDetails={() => toggleDetails("sessions")}
                    />
                  ) : null}
                </section>
              ) : null}

              {groups.hasLoginAndCache ? (
                <section
                  className="space-y-2 rounded-2xl border border-destructive/30 bg-destructive/[0.04] p-3"
                  data-testid="cleanup-irreversible-section"
                >
                  <div className="flex items-center gap-1.5 text-[12px] font-semibold text-destructive-strong">
                    <IconAlert size={13} />
                    <h3>{t("cleanup.sectionIrreversible")}</h3>
                  </div>

                  <OptionCard
                    danger
                    testId="cleanup-login-and-cache"
                    checkboxTestId="cleanup-opt-login-and-cache"
                    checked={effectiveSelection.loginAndCache}
                    onChange={(checked) => toggle("loginAndCache", checked)}
                    title={t("cleanup.catLoginTitle")}
                    dangerBadge={t("cleanup.badgeIrreversible")}
                    runningBadge={activePlan.agentRunning ? t("cleanup.badgeNeedsQuit", { tool: toolName }) : undefined}
                    meta={
                      activePlan.loginInKeychain
                        ? groups.loginCacheExtras.length > 0
                          ? t("cleanup.summaryWithKeychain", {
                              files: loginStats.files,
                              size: formatBytes(loginStats.bytes),
                            })
                          : t("cleanup.summaryKeychainOnly")
                        : t("cleanup.summaryItems", {
                            count: groups.loginCacheExtras.length,
                            files: loginStats.files,
                            size: formatBytes(loginStats.bytes),
                          })
                    }
                    description={t("cleanup.catLoginDesc")}
                    details={groups.loginCacheExtras}
                    keychainIncluded={Boolean(activePlan.loginInKeychain)}
                    detailsKey="loginAndCache"
                    expanded={Boolean(expanded.loginAndCache)}
                    onToggleDetails={() => toggleDetails("loginAndCache")}
                  />
                </section>
              ) : null}

              {activePlan.configDrifted && activePlan.blockers.length === 0 ? (
                <Callout tone="warn" icon={<IconAlert size={14} />}>
                  <span data-testid="cleanup-drifted">{t("cleanup.drifted")}</span>
                </Callout>
              ) : null}

              {activePlan.blockers.length > 0 ? (
                <Callout tone="danger" icon={<IconAlert size={14} />}>
                  <span data-testid="cleanup-blocked">{t("cleanup.repair")}</span>
                </Callout>
              ) : null}

              {!anySelected ? (
                <p className="text-[12.5px] text-muted-foreground" data-testid="cleanup-select-hint">
                  {t("cleanup.selectHint")}
                </p>
              ) : null}

              {needsMemoryAck ? (
                <div className="rounded-xl border border-warning/35 bg-warning/[0.08] px-3 py-2.5">
                  <Checkbox
                    data-testid="cleanup-ack"
                    checked={ackMemory}
                    onChange={(event) => setAckMemory(event.target.checked)}
                    label={t("cleanup.ack", { file: memoryFileName })}
                  />
                </div>
              ) : null}

              {needsEraseAck ? (
                <div className="rounded-xl border border-destructive/40 bg-destructive/[0.08] px-3 py-2.5">
                  <Checkbox
                    data-testid="cleanup-ack-erase"
                    checked={ackErase}
                    onChange={(event) => setAckErase(event.target.checked)}
                    label={
                      <span className="font-medium text-destructive-strong">
                        {t("cleanup.ackErase")}
                      </span>
                    }
                  />
                </div>
              ) : null}

              {needsAgentStopped ? (
                <Callout tone="warn" icon={<IconAlert size={14} />}>
                  <span data-testid="cleanup-running">{t("cleanup.agentRunning", { tool: toolName })}</span>
                </Callout>
              ) : null}

              <p className="flex items-start gap-2 rounded-xl bg-muted/60 px-3 py-2 text-[12px] leading-relaxed text-muted-foreground">
                <IconShield size={13} className="mt-0.5 shrink-0 text-success" />
                <span>
                  {t("cleanup.saves")}
                  <br />
                  {t("cleanup.untouched")}
                </span>
              </p>
            </>
          )}
        </div>
      ) : null}

      {result ? (
        <div className="space-y-3" data-testid="cleanup-done">
          <p className="flex items-center gap-2 text-[13.5px] font-medium text-success">
            <IconCheck size={14} />
            {doneHeadline}
          </p>
          {result.sessionsCleared && result.sessionsCleared > 0 ? (
            <p className="text-[12.5px] text-muted-foreground" data-testid="cleanup-done-sessions">
              {t("cleanup.doneSessionsSaved", { count: result.sessionsCleared })}
            </p>
          ) : null}
          {result.rewriteDisconnected ? (
            <p className="text-[12.5px] text-muted-foreground" data-testid="cleanup-rewrite-disconnected">
              {t("cleanup.rewriteDisconnected")}
            </p>
          ) : null}
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

function OptionCard({
  testId,
  checkboxTestId,
  checked,
  onChange,
  title,
  description,
  meta,
  badge,
  secondaryBadge,
  dangerBadge,
  runningBadge,
  warning,
  danger = false,
  details = [],
  keychainIncluded = false,
  detailsKey,
  expanded,
  onToggleDetails,
}: {
  testId: string;
  checkboxTestId: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  title: string;
  description: string;
  meta?: string;
  badge?: string;
  secondaryBadge?: string;
  dangerBadge?: string;
  runningBadge?: string;
  warning?: string;
  danger?: boolean;
  details?: ExtraMeta[];
  keychainIncluded?: boolean;
  detailsKey: string;
  expanded: boolean;
  onToggleDetails: () => void;
}) {
  const { t } = useTranslation();
  const detailCount = details.length + (keychainIncluded ? 1 : 0);

  return (
    <div
      data-testid={testId}
      className={cx(
        "rounded-xl border px-3 py-2.5 transition-colors",
        danger
          ? checked
            ? "border-destructive/50 bg-card"
            : "border-destructive/20 bg-card/80"
          : checked
            ? "border-primary/45 bg-primary/[0.04]"
            : "border-border bg-card",
      )}
    >
      <label className="flex cursor-pointer items-start gap-2.5">
        <Checkbox
          data-testid={checkboxTestId}
          checked={checked}
          onChange={(event) => onChange(event.target.checked)}
          className="mt-0.5"
        />
        <span className="min-w-0 flex-1">
          <span className="flex flex-wrap items-center gap-1.5">
            <span className="text-[13.5px] font-medium text-foreground">{title}</span>
            {badge ? (
              <span className="rounded-full bg-primary/15 px-1.5 py-0.5 text-[10.5px] font-medium text-primary">
                {badge}
              </span>
            ) : null}
            {secondaryBadge ? (
              <span className="rounded-full bg-success/15 px-1.5 py-0.5 text-[10.5px] font-medium text-success-strong">
                {secondaryBadge}
              </span>
            ) : null}
            {dangerBadge ? (
              <span className="rounded-full bg-destructive/15 px-1.5 py-0.5 text-[10.5px] font-medium text-destructive-strong">
                {dangerBadge}
              </span>
            ) : null}
            {runningBadge ? (
              <span className="rounded-full bg-warning/15 px-1.5 py-0.5 text-[10.5px] font-medium text-warning-strong">
                {runningBadge}
              </span>
            ) : null}
          </span>
          {meta ? <span className="mt-0.5 block text-[12px] text-muted-foreground">{meta}</span> : null}
          <span className="mt-0.5 block text-[12.5px] leading-snug text-muted-foreground">{description}</span>
          {warning ? <span className="mt-1 block text-[12px] text-warning-strong">{warning}</span> : null}
        </span>
      </label>

      {detailCount > 0 ? (
        <div className="mt-2 pl-6">
          <button
            type="button"
            data-testid={`cleanup-details-${detailsKey}`}
            onClick={onToggleDetails}
            className="inline-flex items-center gap-1 text-[11.5px] font-medium text-muted-foreground hover:text-foreground"
          >
            {expanded ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
            {expanded ? t("cleanup.hideDetails") : t("cleanup.showDetails", { count: detailCount })}
          </button>
          {expanded ? (
            <ul className="mt-1.5 space-y-1 rounded-lg bg-muted/50 px-2.5 py-2 text-[12px] text-muted-foreground">
              {keychainIncluded ? (
                <li data-testid="cleanup-keychain" className="text-destructive-strong">
                  {t("cleanup.loginInKeychain")}
                </li>
              ) : null}
              {details.map((extra) => (
                <li
                  key={extra.name}
                  data-testid={extra.kind === "erased" ? "cleanup-extra-erased" : "cleanup-extra"}
                  className="break-all"
                >
                  {t(
                    extra.kind === "erased"
                      ? "cleanup.extraErased"
                      : extra.kind === "saved-without-login"
                        ? "cleanup.extraSavedWithoutLogin"
                        : "cleanup.extraSaved",
                    { name: extra.name, files: extra.files, size: formatBytes(extra.bytes) },
                  )}
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
