import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import type { AppPage } from "../components/AppShell";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { ErrorBanner } from "../components/ErrorBanner";
import { PlanPreview } from "../components/PlanPreview";
import { PromptList } from "../components/PromptList";
import { ScopeBar } from "../components/ScopeBar";
import { ToolStatusBar } from "../components/ToolStatusBar";
import { ZCodeBanner } from "../components/ZCodeBanner";
import { IconPlus, IconSearch } from "../components/icons";
import { Button, Disclosure, Input, Mono, Select, cx } from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import { toastSafeMessage } from "../lib/redact";
import { canConfirmPlan } from "../lib/planGate";
import { pickDirectory } from "../lib/runtime";
import { shortPath } from "../lib/format";
import {
  activeIdsFor,
  defaultScopeFor,
  isRecoveryState,
  mergeTools,
  scopeNeedsProjectDir,
  scopesForTool,
  scopesFromEnvelope,
} from "../lib/tools";
import { isZcodeUnavailable } from "../lib/zcode";
import type {
  Activation,
  Envelope,
  Operation,
  PlanResult,
  PromptSort,
  PromptSummary,
  ScopeId,
  Settings,
  ToolId,
  ToolInfo,
} from "../types";

/**
 * The backend orders by updated / created / title. `lastUsedAt` only exists on
 * the UI summary, so that one ordering is applied here instead of being sent
 * to a backend that would silently fall back to `updated`.
 */
function sortPrompts(prompts: PromptSummary[], sort: PromptSort): PromptSummary[] {
  if (sort !== "lastUsed") return prompts;
  return [...prompts].sort((left, right) => {
    if (!left.lastUsedAt && !right.lastUsedAt) return 0;
    if (!left.lastUsedAt) return 1;
    if (!right.lastUsedAt) return -1;
    return right.lastUsedAt.localeCompare(left.lastUsedAt);
  });
}

export function ToolPage({
  tool,
  settings,
  toast,
  onNavigate,
  onRememberProject,
  onDirtyChange,
  libraryEpoch = 0,
}: {
  tool: ToolId;
  settings: Settings;
  toast: ToastApi;
  onNavigate: (page: AppPage) => void;
  onRememberProject: (dir: string) => void;
  onDirtyChange?: (dirty: boolean) => void;
  libraryEpoch?: number;
}) {
  const { t } = useTranslation();
  const [toolInfo, setToolInfo] = useState<ToolInfo>(
    () => mergeTools(null).find((item) => item.id === tool)!,
  );
  const [status, setStatus] = useState<Envelope | null>(null);
  const [doctorEnv, setDoctorEnv] = useState<Envelope | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [prompts, setPrompts] = useState<PromptSummary[]>([]);
  const [promptsLoading, setPromptsLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState("");
  const [sort, setSort] = useState<PromptSort>("lastUsed");
  const [scope, setScope] = useState<ScopeId>("user");
  const [projectDir, setProjectDir] = useState("");
  const [busy, setBusy] = useState(false);
  const [plan, setPlan] = useState<{ kind: "activate" | "deactivate" | "recover"; result: PlanResult } | null>(
    null,
  );
  const [planError, setPlanError] = useState<string | null>(null);
  const [operations, setOperations] = useState<Operation[]>([]);
  const [activations, setActivations] = useState<Activation[] | null>(null);
  const promptListSeq = useRef(0);
  const statusSeq = useRef(0);
  const toolRef = useRef(tool);
  toolRef.current = tool;

  const supportedScopes = useMemo(() => {
    const fromStatus = scopesFromEnvelope(status, tool);
    if (fromStatus.length > 0) return fromStatus;
    return scopesForTool(tool, toolInfo.supportedScopes);
  }, [status, tool, toolInfo.supportedScopes]);

  const unavailable = isZcodeUnavailable(toolInfo) || status?.available === false;
  const requireProject = scopeNeedsProjectDir(scope);

  const activeIds = useMemo(
    () => (activations ? activeIdsFor(activations, tool, scope, projectDir) : null),
    [activations, projectDir, scope, tool],
  );

  const loadStatus = useCallback(async () => {
    const seq = ++statusSeq.current;
    const args: { tool: ToolId; scope?: ScopeId; projectDir?: string } = { tool, scope };
    if (scopeNeedsProjectDir(scope) && projectDir) args.projectDir = projectDir;
    try {
      const envelope = await api.toolStatus(args);
      if (seq !== statusSeq.current || toolRef.current !== tool) return;
      setStatus(envelope);
      if (envelope.available === false && tool === "zcode") {
        setToolInfo((current) => ({
          ...current,
          available: false,
          unavailableReason: envelope.unavailableReason,
        }));
      }
    } catch (err) {
      if (seq !== statusSeq.current) return;
      setStatus(null);
      setError(toastSafeMessage(err) || t("errors.loadFailed"));
    }
    try {
      const report = await api.doctor(tool);
      if (seq !== statusSeq.current) return;
      setDoctorEnv(report);
    } catch {
      if (seq !== statusSeq.current) return;
      setDoctorEnv(null);
    }
  }, [projectDir, scope, tool]);

  const loadPrompts = useCallback(async () => {
    const seq = ++promptListSeq.current;
    setPromptsLoading(true);
    try {
      const result = await api.listPrompts({
        tool,
        query: query || undefined,
        tag: tag || undefined,
        sort,
      });
      if (seq !== promptListSeq.current || toolRef.current !== tool) return;
      setPrompts(sortPrompts(result.prompts ?? [], sort));
      setError(null);
    } catch (err) {
      if (seq !== promptListSeq.current || toolRef.current !== tool) return;
      setPrompts([]);
      setError(toastSafeMessage(err) || t("errors.apiUnavailable"));
    } finally {
      if (seq === promptListSeq.current) setPromptsLoading(false);
    }
  }, [query, sort, tag, tool]);

  const loadOps = useCallback(async () => {
    try {
      const ops = await api.listOperations(tool);
      setOperations(ops.operations ?? []);
    } catch {
      setOperations([]);
    }
    try {
      const acts = await api.listActivations(tool);
      setActivations(acts.activations ?? []);
    } catch {
      setActivations(null);
    }
  }, [tool]);

  const refreshMeta = useCallback(async () => {
    try {
      const listed = await api.listTools();
      const merged = mergeTools(listed.tools).find((item) => item.id === tool);
      if (merged) setToolInfo(merged);
    } catch {
      setToolInfo(mergeTools(null).find((item) => item.id === tool)!);
    }
    await Promise.all([loadStatus(), loadOps()]);
  }, [loadOps, loadStatus, tool]);

  const refreshAll = useCallback(async () => {
    await Promise.all([refreshMeta(), loadPrompts()]);
  }, [loadPrompts, refreshMeta]);

  useEffect(() => {
    promptListSeq.current += 1;
    statusSeq.current += 1;
    setPlan(null);
    setPlanError(null);
    setQuery("");
    setTag("");
    setPrompts([]);
    setPromptsLoading(true);
    const nextScope = defaultScopeFor(tool, scopesForTool(tool), settings.defaultClaudeScope);
    setScope(nextScope);
    setProjectDir("");
    setToolInfo(mergeTools(null).find((item) => item.id === tool)!);
  }, [settings.defaultClaudeScope, tool]);

  useEffect(() => {
    void refreshMeta();
  }, [refreshMeta]);

  useEffect(() => {
    // Filter keystrokes debounce. Tool switches load immediately so the list
    // cannot stay on "loading" because a later status/doctor request bumped a
    // shared sequence or because `t` changed identity every render.
    const delay = query || tag ? 180 : 0;
    const handle = window.setTimeout(() => {
      void loadPrompts();
    }, delay);
    return () => window.clearTimeout(handle);
  }, [loadPrompts, query, tag, libraryEpoch]);

  const selectPrompt = (id: string) => {
    onNavigate({ kind: "prompt-view", tool, promptId: id, scope, projectDir });
  };

  useEffect(() => {
    onDirtyChange?.(false);
  }, [onDirtyChange]);

  const startCreate = () => {
    onNavigate({ kind: "prompt-edit", tool, creating: true, scope, projectDir });
  };



  const confirmPlan = async () => {
    if (
      !plan ||
      !canConfirmPlan(plan.result.envelope) ||
      (plan.kind !== "recover" && isRecoveryState(plan.result.envelope))
    ) return;
    setBusy(true);
    setPlanError(null);
    try {
      const result =
        plan.kind === "activate"
          ? await api.activate(plan.result.operationId)
          : plan.kind === "deactivate"
            ? await api.deactivate(plan.result.operationId)
            : await api.confirmRecover(plan.result.operationId);
      if (!result.envelope.ok || result.envelope.exitCode !== 0) {
        const reason = toastSafeMessage(result.envelope.error || t("plan.failed"));
        setPlanError(reason);
        toast.err(reason);
      } else {
        toast.ok(t("plan.success"));
        setPlan(null);
      }
      await Promise.all([loadStatus(), loadPrompts(), loadOps()]);
    } catch (err) {
      const reason = toastSafeMessage(err);
      setPlanError(reason);
      toast.err(reason);
    } finally {
      setBusy(false);
    }
  };

  const recover = async () => {
    setBusy(true);
    try {
      const result = await api.recoverTool({
        tool,
        scope,
        projectDir: requireProject ? projectDir || undefined : undefined,
      });
      setPlan({ kind: "recover", result });
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };



  const availableTags = useMemo(() => {
    const set = new Set<string>();
    for (const item of prompts) for (const value of item.tags) set.add(value);
    return [...set].sort();
  }, [prompts]);

  const recovery = isRecoveryState(status);
  const doctorOk = doctorEnv?.doctor?.ok ?? status?.doctor?.ok;
  const filtered = Boolean(query.trim() || tag);

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 overflow-y-auto pr-0.5">
      {error ? (
        <ErrorBanner message={error} onRetry={() => void refreshAll()} retryLabel={t("common.retry")} />
      ) : null}
      <ZCodeBanner tool={toolInfo} />

      <ToolStatusBar
        tool={tool}
        toolInfo={toolInfo}
        status={status}
        doctorOk={doctorOk}
        recovery={recovery}
        unavailable={Boolean(unavailable)}
        busy={busy}
        advancedEnabled={settings.advancedToolsEnabled}
        onRefresh={() => void refreshAll()}
        onRecover={() => void recover()}
      />

      <ScopeBar
        scope={scope}
        supportedScopes={supportedScopes}
        projectDir={projectDir}
        recentProjectDirs={settings.recentProjectDirs}
        disabled={Boolean(unavailable) || busy}
        onScopeChange={setScope}
        onProjectDirChange={setProjectDir}
        onBrowse={() => {
          void (async () => {
            const picked = await pickDirectory();
            if (picked) {
              setProjectDir(picked);
              onRememberProject(picked);
            }
          })();
        }}
      />

      <section className="flex shrink-0 flex-col overflow-hidden rounded-2xl border border-border bg-card">
          <div className="flex flex-col gap-3 border-b border-border px-3 py-3 sm:px-4">
            <div className="flex flex-wrap items-center gap-2">
              <div className="flex min-w-0 items-center gap-2">
                <span className="text-[17px] font-semibold text-foreground">{t("prompts.library")}</span>
                <span className="text-[13px] tabular-nums text-muted-foreground">{prompts.length}</span>
              </div>
              <div className="relative min-w-0 flex-1">
                <IconSearch
                  size={15}
                  className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground"
                />
                <Input
                  value={query}
                  aria-label={t("common.search")}
                  placeholder={t("common.search")}
                  data-testid="prompt-search"
                  onChange={(event) => setQuery(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Escape" && query) setQuery("");
                  }}
                  className="h-10 pl-8"
                />
              </div>
              <Button
                size="md"
                variant="primary"
                disabled={Boolean(unavailable) || busy}
                data-testid="prompt-new"
                title={t("prompts.new")}
                onClick={startCreate}
              >
                <IconPlus />
                <span className="hidden sm:inline">{t("prompts.new")}</span>
              </Button>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Select
                value={tag}
                aria-label={t("prompts.filterTag")}
                className="min-w-[140px] flex-1"
                onChange={(event) => setTag(event.target.value)}
                data-testid="prompt-filter-tag"
              >
                <option value="">{t("prompts.allTags")}</option>
                {availableTags.map((item) => (
                  <option key={item} value={item}>
                    {item}
                  </option>
                ))}
              </Select>
              <Select
                value={sort}
                aria-label={t("prompts.sort")}
                className="min-w-[140px] flex-1"
                onChange={(event) => setSort(event.target.value as PromptSort)}
                data-testid="prompt-sort"
              >
                <option value="lastUsed">{t("prompts.sortLastUsed")}</option>
                <option value="updated">{t("prompts.sortUpdated")}</option>
                <option value="title">{t("prompts.sortTitle")}</option>
                <option value="created">{t("prompts.sortCreated")}</option>
              </Select>
            </div>
          </div>

          <div className="flex flex-col p-3 sm:p-4">
            {unavailable ? (
              <EmptyState
                title={t("tool.unavailable")}
                hint={toolInfo.unavailableReason ?? status?.unavailableReason ?? undefined}
                testId="prompt-list-unavailable"
              />
            ) : (
              <PromptList
                prompts={prompts}
                selectedId={null}
                activeIds={activeIds}
                loading={promptsLoading}
                filtered={filtered}
                onSelect={selectPrompt}
                emptyAction={
                  <Button size="sm" variant="primary" onClick={startCreate}>
                    <IconPlus />
                    {t("prompts.new")}
                  </Button>
                }
              />
            )}
          </div>
      </section>



      {settings.advancedToolsEnabled && operations.length > 0 ? (
        <Disclosure title={t("operations.title")} testId="tool-operations">
          <ul className="flex flex-col gap-1.5 text-[13px]">
            {operations.slice(0, 12).map((item) => (
              <li key={item.id} className="flex flex-wrap items-center gap-2">
                <Mono>{item.kind}</Mono>
                <span
                  className={cx(
                    item.status === "failed" ? "text-destructive" : "text-muted-foreground",
                  )}
                >
                  {item.status}
                </span>
                {item.error ? <span className="text-destructive">{item.error}</span> : null}
                {item.recoverAvailable ? (
                  <Button size="sm" variant="ghost" disabled={busy} onClick={() => void recover()}>
                    {t("operations.restoreEntry")}
                  </Button>
                ) : null}
              </li>
            ))}
          </ul>
        </Disclosure>
      ) : null}

      <ConfirmDialog
        open={Boolean(plan)}
        wide
        title={
          plan?.kind === "deactivate"
            ? t("plan.titleDeactivate")
            : plan?.kind === "recover"
              ? t("operations.recover")
              : t("plan.titleActivate")
        }
        description={planTargetLabel(t, tool, scope, projectDir, undefined)}
        confirmLabel={
          plan?.kind === "deactivate"
            ? t("plan.confirmDeactivate")
            : plan?.kind === "recover"
              ? t("operations.restoreEntry")
              : t("plan.confirmActivate")
        }
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        busy={busy}
        confirmDisabled={
          !plan ||
          !canConfirmPlan(plan.result.envelope) ||
          (plan.kind !== "recover" && isRecoveryState(plan.result.envelope)) ||
          busy
        }
        confirmTestId="plan-confirm"
        onClose={() => {
          setPlan(null);
          setPlanError(null);
        }}
        onConfirm={() => void confirmPlan()}
      >
        {plan ? <PlanPreview envelope={plan.result.envelope} /> : null}
        {planError ? (
          <div className="mt-3 rounded-2xl border border-destructive/40 bg-destructive/10 px-3 py-2 text-destructive">
            <p className="font-medium">{t("plan.failed")}</p>
            <p className="mt-0.5">{planError}</p>
            <Button className="mt-2" size="sm" onClick={() => void recover()}>
              {t("operations.restoreEntry")}
            </Button>
          </div>
        ) : null}
      </ConfirmDialog>
    </div>
  );
}

function planTargetLabel(
  t: (key: string) => string,
  tool: ToolId,
  scope: ScopeId,
  projectDir: string,
  promptTitle: string | undefined,
): string {
  const parts = [t(`nav.${tool}`), t(`scope.${scope}`)];
  if (scopeNeedsProjectDir(scope) && projectDir) parts.push(shortPath(projectDir, 40));
  if (promptTitle) parts.push(promptTitle);
  return parts.join(" · ");
}
