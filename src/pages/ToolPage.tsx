import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { ErrorBanner } from "../components/ErrorBanner";
import { PlanPreview } from "../components/PlanPreview";
import { PromptEditor, type PromptDraft } from "../components/PromptEditor";
import { PromptFormPanel } from "../components/PromptFormPanel";
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
  PromptDetail,
  PromptSort,
  PromptSummary,
  PromptVersion,
  ScopeId,
  Settings,
  ToolId,
  ToolInfo,
} from "../types";

const EMPTY_DRAFT: PromptDraft = { title: "", content: "", tags: "" };

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
  onRememberProject,
  onDirtyChange,
  libraryEpoch = 0,
}: {
  tool: ToolId;
  settings: Settings;
  toast: ToastApi;
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
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<PromptDetail | null>(null);
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState("");
  const [sort, setSort] = useState<PromptSort>("lastUsed");
  const [scope, setScope] = useState<ScopeId>("user");
  const [projectDir, setProjectDir] = useState("");
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState(false);
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState<PromptDraft>(EMPTY_DRAFT);
  const [plan, setPlan] = useState<{ kind: "activate" | "deactivate" | "recover"; result: PlanResult } | null>(
    null,
  );
  const [planError, setPlanError] = useState<string | null>(null);
  const [versions, setVersions] = useState<PromptVersion[]>([]);
  const [diff, setDiff] = useState("");
  const [operations, setOperations] = useState<Operation[]>([]);
  // null = activation table could not be read. Never collapse this to [], or every
  // activated prompt silently renders as inactive and invites a duplicate activate.
  const [activations, setActivations] = useState<Activation[] | null>(null);
  const [formOpen, setFormOpen] = useState(false);
  const promptListSeq = useRef(0);
  const promptDetailSeq = useRef(0);
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
  const projectReady = !requireProject || Boolean(projectDir.trim());

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
    promptDetailSeq.current += 1;
    statusSeq.current += 1;
    setSelectedId(null);
    setDetail(null);
    setCreating(false);
    setEditing(false);
    setFormOpen(false);
    setPlan(null);
    setPlanError(null);
    setQuery("");
    setTag("");
    setVersions([]);
    setDiff("");
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

  const selectPrompt = async (id: string) => {
    const seq = ++promptDetailSeq.current;
    setSelectedId(id);
    setCreating(false);
    setEditing(false);
    setFormOpen(false);
    setDiff("");
    try {
      const next = await api.getPrompt(id);
      if (seq !== promptDetailSeq.current) return;
      setDetail(next);
      setDraft({ title: next.title, content: next.content, tags: next.tags.join(", ") });
      const history = await api.promptHistory(id);
      if (seq !== promptDetailSeq.current) return;
      setVersions(history.versions ?? []);
    } catch (err) {
      toast.err(err);
    }
  };

  const tagsOf = (value: string) =>
    value
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean);

  const isDirty = creating
    ? Boolean(draft.title || draft.content || draft.tags)
    : Boolean(
        detail &&
          (draft.title !== detail.title ||
            draft.content !== detail.content ||
            draft.tags !== detail.tags.join(", ")),
      );

  const confirmLeave = () => {
    if (!isDirty) return true;
    return window.confirm(t("unsaved.leave"));
  };

  useEffect(() => {
    onDirtyChange?.(isDirty);
  }, [isDirty, onDirtyChange]);

  const startCreate = () => {
    if (!confirmLeave()) return;
    setCreating(true);
    setEditing(true);
    setFormOpen(true);
    setSelectedId(null);
    setDetail(null);
    setDraft(EMPTY_DRAFT);
    setVersions([]);
    setDiff("");
  };

  const cancelEdit = () => {
    if (creating) {
      setCreating(false);
      setEditing(false);
      setDraft(EMPTY_DRAFT);
      return;
    }
    setEditing(false);
    if (detail) {
      setDraft({ title: detail.title, content: detail.content, tags: detail.tags.join(", ") });
    }
  };

  const saveDraft = async () => {
    if (!draft.title.trim()) {
      toast.err(t("errors.validation"));
      return;
    }
    setBusy(true);
    try {
      if (creating) {
        const created = await api.createPrompt({
          tool,
          title: draft.title.trim(),
          content: draft.content,
          tags: tagsOf(draft.tags),
        });
        toast.ok(t("prompts.created"));
        setCreating(false);
        setEditing(false);
        setFormOpen(false);
        await loadPrompts();
        await selectPrompt(created.id);
      } else if (detail) {
        const updated = await api.updatePrompt({
          id: detail.id,
          title: draft.title.trim(),
          content: draft.content,
          tags: tagsOf(draft.tags),
        });
        toast.ok(t("prompts.updatedOk"));
        setDetail(updated);
        setEditing(false);
        setFormOpen(false);
        await loadPrompts();
      }
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const duplicateSameTool = async () => {
    if (!detail) return;
    setBusy(true);
    try {
      const created = await api.createPrompt({
        tool,
        title: `${detail.title} copy`,
        content: detail.content,
        tags: detail.tags,
      });
      toast.ok(t("prompts.created"));
      await loadPrompts();
      await selectPrompt(created.id);
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const copyTo = async (targetTool: ToolId) => {
    if (!detail) return;
    setBusy(true);
    try {
      await api.copyPrompt(detail.id, targetTool);
      toast.ok(t("prompts.copied"));
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const removePrompt = async () => {
    if (!detail) return;
    if (!window.confirm(t("prompts.deleteConfirm"))) return;
    setBusy(true);
    try {
      await api.deletePrompt(detail.id);
      toast.ok(t("prompts.deleted"));
      setDetail(null);
      setSelectedId(null);
      setVersions([]);
      setDiff("");
      await loadPrompts();
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const openPlan = async (kind: "activate" | "deactivate") => {
    if (unavailable) return;
    if (!projectReady) {
      toast.err(t("scope.needsProjectDir"));
      return;
    }
    if (kind === "activate" && !detail) {
      toast.err(t("errors.validation"));
      return;
    }
    setBusy(true);
    setPlanError(null);
    try {
      const result =
        kind === "activate"
          ? await api.planActivate({
              promptId: detail!.id,
              scope,
              projectDir: requireProject ? projectDir : undefined,
            })
          : await api.planDeactivate({
              promptId: detail?.id,
              tool,
              scope,
              projectDir: requireProject ? projectDir : undefined,
            });
      setPlan({ kind, result });
      if (requireProject && projectDir) onRememberProject(projectDir);
    } catch (err) {
      setPlan(null);
      toast.err(err);
    } finally {
      setBusy(false);
    }
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
      if (detail) await selectPrompt(detail.id);
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

  const restoreVersion = async (version: number) => {
    if (!detail) return;
    setBusy(true);
    try {
      const restored = await api.restorePromptVersion(detail.id, version);
      toast.ok(t("prompts.restored"));
      setDetail(restored);
      setDraft({ title: restored.title, content: restored.content, tags: restored.tags.join(", ") });
      await loadPrompts();
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const showDiff = async (fromVersion: number, toVersion: number) => {
    if (!detail) return;
    try {
      const result = await api.promptDiff(detail.id, fromVersion, toVersion);
      setDiff(result.unified || result.summary || "");
    } catch (err) {
      toast.err(err);
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
  const selectedIsActive = detail && activeIds ? activeIds.includes(detail.id) : null;

  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
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

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(420px,1.1fr)_minmax(320px,0.9fr)] gap-3">
        <section className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
          <div className="flex shrink-0 flex-col gap-1.5 border-b border-border px-2.5 py-2">
            <div className="flex items-center gap-1.5">
              <div className="relative min-w-0 flex-1">
                <IconSearch
                  size={13}
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
                  className="pl-7"
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
                <span className="hidden xl:inline">{t("prompts.new")}</span>
              </Button>
            </div>
            <div className="flex items-center gap-1.5">
              <Select
                value={tag}
                aria-label={t("prompts.filterTag")}
                className="min-w-0 flex-1"
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
                className="min-w-0 flex-1"
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

          <div className="flex min-h-0 flex-1 flex-col overflow-hidden p-2">
            {unavailable ? (
              <EmptyState
                title={t("tool.unavailable")}
                hint={toolInfo.unavailableReason ?? status?.unavailableReason ?? undefined}
                testId="prompt-list-unavailable"
              />
            ) : (
              <PromptList
                prompts={prompts}
                selectedId={selectedId}
                activeIds={activeIds}
                loading={promptsLoading}
                filtered={filtered}
                onSelect={(id) => void selectPrompt(id)}
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

        <section className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-card">
          {creating || detail ? (
            <PromptEditor
              tool={tool}
              detail={detail}
              draft={draft}
              creating={creating}
              editing={editing}
              busy={busy}
              disabled={Boolean(unavailable) || !projectReady}
              isActiveHere={selectedIsActive}
              versions={versions}
              diff={diff}
              onDraftChange={setDraft}
              onStartEdit={() => {
                setEditing(true);
                setFormOpen(true);
              }}
              onCancelEdit={cancelEdit}
              onSave={() => void saveDraft()}
              onDuplicate={() => void duplicateSameTool()}
              onCopyTo={(target) => void copyTo(target)}
              onDelete={() => void removePrompt()}
              onActivate={() => void openPlan("activate")}
              onDeactivate={() => void openPlan("deactivate")}
              onRestoreVersion={(version) => void restoreVersion(version)}
              onShowDiff={(from, to) => void showDiff(from, to)}
            />
          ) : (
            <div className="flex min-h-0 flex-1 items-center justify-center p-4">
              <EmptyState
                title={t("prompts.noSelection")}
                hint={t("prompts.noSelectionHint")}
                testId="prompt-no-selection"
              />
            </div>
          )}
        </section>
      </div>

      {settings.advancedToolsEnabled && operations.length > 0 ? (
        <Disclosure title={t("operations.title")} testId="tool-operations">
          <ul className="flex flex-col gap-1 text-[11px]">
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
        title={
          plan?.kind === "deactivate"
            ? t("plan.titleDeactivate")
            : plan?.kind === "recover"
              ? t("operations.recover")
              : t("plan.titleActivate")
        }
        description={planTargetLabel(t, tool, scope, projectDir, detail?.title)}
        confirmLabel={
          plan?.kind === "deactivate"
            ? t("plan.confirmDeactivate")
            : plan?.kind === "recover"
              ? t("operations.restoreEntry")
              : t("plan.confirmActivate")
        }
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
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
          <div className="mt-3 rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-destructive">
            <p className="font-medium">{t("plan.failed")}</p>
            <p className="mt-0.5">{planError}</p>
            <Button className="mt-2" size="sm" onClick={() => void recover()}>
              {t("operations.restoreEntry")}
            </Button>
          </div>
        ) : null}
      </ConfirmDialog>
      {formOpen ? (
        <PromptFormPanel
          title={creating ? t("prompts.new") : t("prompts.edit")}
          draft={draft}
          saving={busy}
          onChange={setDraft}
          onSave={() => void saveDraft()}
          onClose={() => {
            if (!confirmLeave()) return;
            cancelEdit();
            setFormOpen(false);
          }}
          dirtyGuard={confirmLeave}
        />
      ) : null}
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
