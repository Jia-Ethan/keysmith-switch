import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { AgentHero } from "../components/AgentHero";
import type { AppPage } from "../components/AppShell";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { DeployCelebration } from "../components/DeployCelebration";
import { Dropdown } from "../components/Dropdown";
import { EmptyState } from "../components/EmptyState";
import { Callout, PlanPreview } from "../components/PlanPreview";
import { PromptList } from "../components/PromptList";
import { QuickDeployPanel } from "../components/QuickDeployPanel";
import { ScopeBar } from "../components/ScopeBar";
import { ToolLogo } from "../components/ToolLogos";
import { ZCodeBanner } from "../components/ZCodeBanner";
import { IconAlert, IconPlus, IconSearch } from "../components/icons";
import { Button, Disclosure, Input, Mono, cx } from "../components/ui";
import { useHotkeys } from "../hooks/useHotkeys";
import type { ToastApi } from "../hooks/useToasts";
import { shortPath } from "../lib/format";
import { applyHarnessOutcome, getHarnessStatus, loadHarnessStatus, useHarnessStatus } from "../lib/harnessState";
import { shortcutLabel } from "../lib/platform";
import { canConfirmPlan } from "../lib/planGate";
import { toastSafeMessage } from "../lib/redact";
import { pickDirectory } from "../lib/runtime";
import { activeIdsFor, defaultScopeFor, isRecoveryState, mergeTools, scopeNeedsProjectDir } from "../lib/tools";
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

type PlanKind = "activate" | "deactivate" | "recover";

interface OpenPlan {
  kind: PlanKind;
  result: PlanResult;
  scope: ScopeId;
  projectDir: string;
  title: string;
  /** Plans opened from the composer keep the draft dirty until they resolve. */
  source: "composer" | "library" | "machine";
}

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

/**
 * One agent's workspace: the live deployment on top, the prompt library below,
 * and the Quick Deploy composer one shortcut away. Every write still goes
 * through a reviewed adapter plan.
 *
 * The page is keyed by tool in the app, so switching agents starts from clean
 * state; the machine read itself is remembered per run in `harnessState`.
 */
export function WorkspacePage({
  tool,
  settings,
  toast,
  onNavigate,
  onRememberProject,
  onDirtyChange,
  libraryEpoch = 0,
}: {
  tool: ToolId;
  settings?: Settings;
  toast?: ToastApi;
  onNavigate?: (page: AppPage) => void;
  onRememberProject?: (dir: string) => void;
  onDirtyChange?: (dirty: boolean) => void;
  libraryEpoch?: number;
}) {
  const { t } = useTranslation();
  const entry = useHarnessStatus(tool);
  const defaultTitle = t("quickDeploy.defaultTitle");
  const [toolInfo, setToolInfo] = useState<ToolInfo>(() => mergeTools(null).find((item) => item.id === tool)!);
  const [projectStatus, setProjectStatus] = useState<Envelope | null>(null);
  const [projectStatusError, setProjectStatusError] = useState<string | null>(null);
  const [scope, setScope] = useState<ScopeId>("user");
  const [projectDir, setProjectDir] = useState("");
  const [reading, setReading] = useState(false);
  const [statusEpoch, setStatusEpoch] = useState(0);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [prompts, setPrompts] = useState<PromptSummary[]>([]);
  const [promptsLoading, setPromptsLoading] = useState(true);
  const [promptsError, setPromptsError] = useState<string | null>(null);
  const [activations, setActivations] = useState<Activation[] | null>(null);
  const [operations, setOperations] = useState<Operation[]>([]);
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState("");
  const [sort, setSort] = useState<PromptSort>("lastUsed");
  const [localEpoch, setLocalEpoch] = useState(0);

  const [composerOpen, setComposerOpen] = useState(false);
  const [title, setTitle] = useState(defaultTitle);
  const [content, setContent] = useState("");
  const [savedDraft, setSavedDraft] = useState<string | null>(null);
  const [composerMessage, setComposerMessage] = useState<string | null>(null);

  const [plan, setPlan] = useState<OpenPlan | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [celebration, setCelebration] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const promptSeq = useRef(0);

  const supportedScopes = useMemo(
    () => (toolInfo.supportedScopes.length ? toolInfo.supportedScopes : (["user"] as ScopeId[])),
    [toolInfo],
  );
  const requireProject = scopeNeedsProjectDir(scope);
  const trimmedDir = projectDir.trim();
  const projectReady = !requireProject || Boolean(trimmedDir);
  const unavailable = isZcodeUnavailable(toolInfo) || !toolInfo.available || projectStatus?.available === false;

  const machine =
    scope === "user"
      ? entry?.error
        ? "unknown"
        : entry?.machine ?? "unknown"
      : projectStatus && projectStatus.available
        ? projectStatus.status === "active"
          ? "deployed"
          : projectStatus.status === "inactive" || projectStatus.status === "not-installed"
            ? "undeployed"
            : "unknown"
        : "unknown";
  const statusError = loadError ?? (scope === "user" ? entry?.error : projectStatusError ?? projectStatus?.error);
  const recovery = scope !== "user" && isRecoveryState(projectStatus);

  const activeIds = useMemo(
    () => (activations ? activeIdsFor(activations, tool, scope, trimmedDir) : null),
    [activations, scope, tool, trimmedDir],
  );
  const deployedTitle = useMemo(() => {
    if (!activeIds?.length) return null;
    const hit = prompts.find((item) => activeIds.includes(item.id));
    if (hit) return hit.title;
    const activation = activations?.find((item) => item.promptId && activeIds.includes(item.promptId));
    return activation?.promptTitle ?? null;
  }, [activations, activeIds, prompts]);

  const draftDirty =
    (title !== defaultTitle || Boolean(content)) && savedDraft !== `${title}\0${content}`;
  const dirty = draftDirty || plan?.source === "composer" || busy;

  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);

  useEffect(() => () => onDirtyChange?.(false), [onDirtyChange]);

  // Tool metadata and the default scope.
  useEffect(() => {
    let cancelled = false;
    void api
      .listTools()
      .then(({ tools }) => {
        if (cancelled) return;
        const next = mergeTools(tools).find((item) => item.id === tool);
        if (!next) return;
        setToolInfo(next);
        setScope(defaultScopeFor(tool, next.supportedScopes, settings?.defaultClaudeScope ?? "user"));
      })
      .catch((reason: unknown) => {
        if (!cancelled) setLoadError(toastSafeMessage(reason) || t("errors.loadFailed"));
      });
    return () => {
      cancelled = true;
    };
  }, [settings?.defaultClaudeScope, tool]);

  // User scope: read the machine once per run.
  useEffect(() => {
    if (getHarnessStatus(tool)) return;
    let cancelled = false;
    setReading(true);
    void loadHarnessStatus(tool).finally(() => {
      if (!cancelled) setReading(false);
    });
    return () => {
      cancelled = true;
    };
  }, [tool]);

  // Project / local scope: status for the chosen directory, debounced while typing.
  useEffect(() => {
    setProjectStatus(null);
    setProjectStatusError(null);
    if (!requireProject || !trimmedDir) return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void api
        .toolStatus({ tool, scope, projectDir: trimmedDir })
        .then((result) => {
          if (!cancelled) setProjectStatus(result);
        })
        .catch((reason: unknown) => {
          if (!cancelled) setProjectStatusError(toastSafeMessage(reason) || t("errors.loadFailed"));
        });
    }, 200);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [requireProject, scope, statusEpoch, tool, trimmedDir]);

  const loadPrompts = useCallback(async () => {
    const seq = ++promptSeq.current;
    setPromptsLoading(true);
    try {
      const result = await api.listPrompts({ tool, query: query || undefined, tag: tag || undefined, sort });
      if (seq !== promptSeq.current) return;
      setPrompts(sortPrompts(result.prompts ?? [], sort));
      setPromptsError(null);
    } catch (reason) {
      if (seq !== promptSeq.current) return;
      setPrompts([]);
      setPromptsError(toastSafeMessage(reason) || t("errors.apiUnavailable"));
    } finally {
      if (seq === promptSeq.current) setPromptsLoading(false);
    }
  }, [query, sort, tag, tool]);

  const loadActivations = useCallback(async () => {
    try {
      const result = await api.listActivations(tool);
      setActivations(result.activations ?? []);
    } catch {
      setActivations(null);
    }
  }, [tool]);

  const loadOperations = useCallback(async () => {
    if (!settings?.advancedToolsEnabled) return;
    try {
      const result = await api.listOperations(tool);
      setOperations(result.operations ?? []);
    } catch {
      setOperations([]);
    }
  }, [settings?.advancedToolsEnabled, tool]);

  useEffect(() => {
    // Filter keystrokes debounce; everything else loads immediately.
    const delay = query || tag ? 180 : 0;
    const handle = window.setTimeout(() => {
      void loadPrompts();
    }, delay);
    return () => window.clearTimeout(handle);
  }, [loadPrompts, query, tag, libraryEpoch, localEpoch]);

  useEffect(() => {
    void loadActivations();
    void loadOperations();
  }, [loadActivations, loadOperations, libraryEpoch, localEpoch]);

  const refresh = async () => {
    setReading(true);
    setLoadError(null);
    try {
      if (scope === "user") await loadHarnessStatus(tool, true);
      else setStatusEpoch((value) => value + 1);
      setLocalEpoch((value) => value + 1);
    } finally {
      setReading(false);
    }
  };

  const fail = (reason: unknown, fallback: string) => {
    const message = toastSafeMessage(reason) || fallback;
    toast?.err(message);
    return message;
  };

  const openComposer = () => {
    if (unavailable || busy) return;
    setComposerMessage(null);
    setComposerOpen(true);
  };

  const resetDraft = () => {
    setTitle(defaultTitle);
    setContent("");
    setSavedDraft(null);
    setComposerMessage(null);
  };

  const validateDraft = (): boolean => {
    if (!title.trim() || !content.trim()) {
      const reason = t("quickDeploy.validation");
      setComposerMessage(reason);
      toast?.err(reason);
      return false;
    }
    return true;
  };

  const previewDraft = async () => {
    if (busy || !validateDraft()) return;
    if (!projectReady) {
      const reason = t("scope.needsProjectDir");
      setComposerMessage(reason);
      toast?.err(reason);
      return;
    }
    setBusy(true);
    setComposerMessage(null);
    try {
      const plannedScope = scope;
      const plannedDir = requireProject ? trimmedDir : "";
      const prompt = await api.createPastedPrompt({ tool, title: title.trim(), content });
      setSavedDraft(`${title}\0${content}`);
      const result = await api.planActivate({
        promptId: prompt.id,
        scope: plannedScope,
        projectDir: plannedDir || undefined,
      });
      setPlanError(null);
      setPlan({ kind: "activate", result, scope: plannedScope, projectDir: plannedDir, title: prompt.title, source: "composer" });
    } catch (reason) {
      setComposerMessage(fail(reason, t("quickDeploy.failed")));
    } finally {
      setBusy(false);
    }
  };

  const saveDraftOnly = async () => {
    if (busy || !validateDraft()) return;
    setBusy(true);
    setComposerMessage(null);
    try {
      await api.createPastedPrompt({ tool, title: title.trim(), content });
      toast?.ok(t("quickDeploy.savedToLibrary"));
      resetDraft();
      setComposerOpen(false);
      setLocalEpoch((value) => value + 1);
    } catch (reason) {
      setComposerMessage(fail(reason, t("errors.saveFailed")));
    } finally {
      setBusy(false);
    }
  };

  const deployFromLibrary = async (promptId: string) => {
    if (busy) return;
    if (!projectReady) {
      toast?.err(t("scope.needsProjectDir"));
      return;
    }
    setBusy(true);
    try {
      const plannedDir = requireProject ? trimmedDir : "";
      const result = await api.planActivate({ promptId, scope, projectDir: plannedDir || undefined });
      const summary = prompts.find((item) => item.id === promptId);
      setPlanError(null);
      setPlan({ kind: "activate", result, scope, projectDir: plannedDir, title: summary?.title ?? "", source: "library" });
    } catch (reason) {
      fail(reason, t("errors.planFailed"));
    } finally {
      setBusy(false);
    }
  };

  const openRemovePlan = async () => {
    if (!projectReady || busy) return;
    setBusy(true);
    try {
      const plannedDir = requireProject ? trimmedDir : "";
      const result = await api.planDeactivate({ tool, scope, projectDir: plannedDir || undefined });
      setPlanError(null);
      setPlan({ kind: "deactivate", result, scope, projectDir: plannedDir, title: deployedTitle ?? "", source: "machine" });
    } catch (reason) {
      fail(reason, t("quickDeploy.failed"));
    } finally {
      setBusy(false);
    }
  };

  const openRecoverPlan = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const plannedScope = plan?.scope ?? scope;
      const plannedDir = plan?.projectDir ?? (requireProject ? trimmedDir : "");
      const result = await api.recoverTool({ tool, scope: plannedScope, projectDir: plannedDir || undefined });
      setPlanError(null);
      setPlan({ kind: "recover", result, scope: plannedScope, projectDir: plannedDir, title: "", source: "machine" });
    } catch (reason) {
      fail(reason, t("errors.recoverFailed"));
    } finally {
      setBusy(false);
    }
  };

  const planBlocked = (candidate: OpenPlan | null) =>
    !candidate ||
    !canConfirmPlan(candidate.result.envelope) ||
    (candidate.kind !== "recover" && isRecoveryState(candidate.result.envelope));

  const confirmPlan = async () => {
    if (!plan || planBlocked(plan)) return;
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
        const message = toastSafeMessage(result.envelope.error || t("plan.failed"));
        setPlanError(message);
        toast?.err(message);
        return;
      }
      if (plan.scope === "user") {
        if (plan.kind === "recover") void loadHarnessStatus(tool, true);
        else applyHarnessOutcome(tool, plan.kind === "activate" ? "deploy" : "remove");
      } else {
        setStatusEpoch((value) => value + 1);
      }
      if (plan.source === "composer") {
        resetDraft();
        setComposerOpen(false);
      }
      if (plan.kind === "activate") setCelebration(plan.title || t("quickDeploy.deployed"));
      else toast?.ok(plan.kind === "deactivate" ? t("quickDeploy.removed") : t("plan.success"));
      setPlan(null);
      setLocalEpoch((value) => value + 1);
    } catch (reason) {
      setPlanError(fail(reason, t("quickDeploy.failed")));
    } finally {
      setBusy(false);
    }
  };

  const closePlan = () => {
    if (busy) return;
    setPlan(null);
    setPlanError(null);
  };

  useHotkeys([
    { key: "n", mod: true, handler: () => openComposer() },
    { key: "/", handler: () => searchRef.current?.focus() },
    { key: "f", mod: true, handler: () => searchRef.current?.focus() },
    { key: "r", mod: true, handler: () => void refresh() },
  ], !composerOpen);

  const availableTags = useMemo(() => {
    const set = new Set<string>();
    for (const item of prompts) for (const value of item.tags) set.add(value);
    if (tag) set.add(tag);
    return [...set].sort();
  }, [prompts, tag]);

  const selectPrompt = (id: string) => {
    onNavigate?.({ kind: "prompt-view", tool, promptId: id, scope, projectDir: trimmedDir });
  };

  const filtered = Boolean(query.trim() || tag);
  const locked = busy || Boolean(plan);
  const toolName = t(`nav.${tool}`);
  const planRecoverable = plan && plan.kind !== "recover" && isRecoveryState(plan.result.envelope);

  return (
    <section
      data-testid="workspace-page"
      data-machine={machine}
      className="h-full min-h-0 w-full overflow-y-auto"
    >
      <div className="mx-auto flex w-full max-w-[1120px] flex-col gap-5 px-4 pb-10 pt-5 sm:px-6">
        <AgentHero
          tool={tool}
          name={toolName}
          machine={machine}
          reading={reading}
          hint={requireProject && !projectReady ? t("hero.pickProject") : null}
          statusError={statusError}
          scope={scope}
          deployedTitle={deployedTitle}
          unavailable={unavailable}
          busy={locked}
          onRefresh={() => void refresh()}
          onRemove={() => void openRemovePlan()}
          removeDisabled={locked || !projectReady}
        >
          <ScopeBar
            embedded
            scope={scope}
            supportedScopes={supportedScopes}
            projectDir={projectDir}
            recentProjectDirs={settings?.recentProjectDirs ?? []}
            disabled={locked || unavailable}
            onScopeChange={(next) => {
              setScope(next);
              setLoadError(null);
            }}
            onProjectDirChange={(dir) => {
              setProjectDir(dir);
              setLoadError(null);
            }}
            onBrowse={() => {
              void pickDirectory().then((dir) => {
                if (dir) {
                  setProjectDir(dir);
                  onRememberProject?.(dir);
                }
              });
            }}
          />
        </AgentHero>

        <ZCodeBanner tool={toolInfo} />

        {recovery ? (
          <div className="flex flex-wrap items-center gap-3 rounded-xl border border-warning/35 bg-warning/10 px-4 py-2.5 text-[12.5px] text-warning" data-testid="tool-recovery-notice">
            <IconAlert size={14} className="shrink-0" />
            <span className="min-w-0 flex-1">{t("tool.recoveryRequired")}</span>
            <Button size="xs" variant="outline" disabled={locked} data-testid="tool-recover" onClick={() => void openRecoverPlan()}>
              {t("tool.recoveryAction")}
            </Button>
          </div>
        ) : null}

        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <div className="relative min-w-[200px] flex-1">
              <IconSearch
                size={15}
                className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
              />
              <Input
                ref={searchRef}
                value={query}
                aria-label={t("common.search")}
                placeholder={t("prompts.searchPlaceholder")}
                data-testid="prompt-search"
                onChange={(event) => setQuery(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    if (query) setQuery("");
                    else event.currentTarget.blur();
                  }
                }}
                className="pl-9 pr-10"
              />
              {!query ? (
                <kbd className="kbd pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground">/</kbd>
              ) : null}
            </div>
            <Dropdown<PromptSort>
              label={t("prompts.sort")}
              testId="prompt-sort"
              className="w-[9.5rem]"
              value={sort}
              onChange={setSort}
              options={[
                { value: "lastUsed", label: t("prompts.sortLastUsed") },
                { value: "updated", label: t("prompts.sortUpdated") },
                { value: "title", label: t("prompts.sortTitle") },
                { value: "created", label: t("prompts.sortCreated") },
              ]}
            />
            <Button
              variant="primary"
              disabled={unavailable || locked}
              data-testid="quick-deploy-open"
              title={`${t("quickDeploy.open")} (${shortcutLabel("n")})`}
              onClick={openComposer}
            >
              <IconPlus size={15} />
              {t("quickDeploy.open")}
              {draftDirty ? (
                <span className="ml-0.5 h-1.5 w-1.5 rounded-full bg-primary-foreground" aria-label={t("quickDeploy.draftPending")} />
              ) : null}
            </Button>
          </div>

          {availableTags.length > 0 ? (
            <div className="flex flex-wrap items-center gap-1.5" role="group" aria-label={t("prompts.filterTag")} data-testid="prompt-filter-tag">
              {["", ...availableTags].map((value) => {
                const active = tag === value;
                return (
                  <button
                    key={value || "__all"}
                    type="button"
                    aria-pressed={active}
                    onClick={() => setTag(value)}
                    className={cx(
                      "h-7 rounded-full px-3 text-[12px] font-medium transition-colors",
                      "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                      active
                        ? "bg-foreground text-background"
                        : "border border-border bg-card text-muted-foreground hover:border-foreground/25 hover:text-foreground",
                    )}
                  >
                    {value ? `#${value}` : t("prompts.allTags")}
                  </button>
                );
              })}
            </div>
          ) : null}
        </div>

        {promptsError ? (
          <Callout tone="danger" icon={<IconAlert size={14} />}>
            <span>{promptsError}</span>{" "}
            <button type="button" className="font-medium underline" onClick={() => setLocalEpoch((value) => value + 1)}>
              {t("common.retry")}
            </button>
          </Callout>
        ) : null}

        {unavailable ? (
          <EmptyState
            icon={<ToolLogo tool={tool} size={22} />}
            title={t("tool.unavailable")}
            hint={toolInfo.unavailableReason ?? projectStatus?.unavailableReason ?? undefined}
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
            onDeploy={(id) => void deployFromLibrary(id)}
            deployDisabled={locked || !projectReady}
            emptyAction={
              <Button variant="primary" onClick={openComposer} data-testid="prompt-empty-compose">
                <IconPlus size={15} />
                {t("quickDeploy.firstPrompt")}
              </Button>
            }
          />
        )}

        {settings?.advancedToolsEnabled && operations.length > 0 ? (
          <Disclosure title={t("operations.title")} testId="tool-operations">
            <ul className="flex flex-col gap-1.5 text-[12.5px]">
              {operations.slice(0, 12).map((item) => (
                <li key={item.id} className="flex flex-wrap items-center gap-2">
                  <Mono>{item.kind}</Mono>
                  <span className={cx(item.status === "failed" ? "text-destructive" : "text-muted-foreground")}>
                    {item.status}
                  </span>
                  {item.error ? <span className="text-destructive">{item.error}</span> : null}
                  {item.recoverAvailable ? (
                    <Button size="xs" variant="ghost" disabled={locked} onClick={() => void openRecoverPlan()}>
                      {t("operations.restoreEntry")}
                    </Button>
                  ) : null}
                </li>
              ))}
            </ul>
          </Disclosure>
        ) : null}
      </div>

      <QuickDeployPanel
        open={composerOpen}
        tool={tool}
        toolName={toolName}
        scope={scope}
        title={title}
        content={content}
        busy={busy}
        message={composerMessage ?? (requireProject && !projectReady ? t("scope.needsProjectDir") : null)}
        messageTone={composerMessage ? "error" : "muted"}
        canDeploy={toolInfo.available}
        defaultTitle={defaultTitle}
        onTitleChange={(value) => {
          setTitle(value);
          setComposerMessage(null);
        }}
        onContentChange={(value) => {
          setContent(value);
          setComposerMessage(null);
        }}
        onPreview={() => void previewDraft()}
        onSaveOnly={() => void saveDraftOnly()}
        onClose={() => {
          if (!busy) setComposerOpen(false);
        }}
      />

      <ConfirmDialog
        open={Boolean(plan)}
        wide
        icon={<ToolLogo tool={tool} size={22} />}
        title={
          plan?.kind === "deactivate"
            ? t("plan.titleDeactivate")
            : plan?.kind === "recover"
              ? t("operations.recover")
              : t("quickDeploy.previewTitle")
        }
        description={[
          toolName,
          t(`scope.${plan?.scope ?? scope}`),
          plan?.projectDir ? shortPath(plan.projectDir, 40) : "",
          plan?.title,
        ]
          .filter(Boolean)
          .join(" · ")}
        confirmLabel={
          plan?.kind === "deactivate"
            ? t("plan.confirmDeactivate")
            : plan?.kind === "recover"
              ? t("operations.restoreEntry")
              : t("quickDeploy.confirm")
        }
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        busy={busy}
        danger={plan?.kind === "deactivate"}
        confirmDisabled={planBlocked(plan) || busy}
        confirmTestId="quick-deploy-confirm"
        onClose={closePlan}
        onConfirm={() => void confirmPlan()}
        footerStart={
          planRecoverable ? (
            <Button size="sm" variant="ghost" disabled={busy} data-testid="plan-recover" onClick={() => void openRecoverPlan()}>
              {t("operations.restoreEntry")}
            </Button>
          ) : null
        }
      >
        {plan ? <PlanPreview envelope={plan.result.envelope} /> : null}
        {planError ? (
          <div className="mt-3">
            <Callout tone="danger" icon={<IconAlert size={14} />}>
              <p className="font-medium">{t("plan.failed")}</p>
              <p className="mt-0.5">{planError}</p>
            </Callout>
          </div>
        ) : null}
      </ConfirmDialog>

      {celebration ? (
        <DeployCelebration
          tool={tool}
          title={t("quickDeploy.deployedTo", { tool: toolName })}
          subtitle={celebration}
          onDone={() => setCelebration(null)}
        />
      ) : null}
    </section>
  );
}
