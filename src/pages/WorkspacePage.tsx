import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { AgentHero } from "../components/AgentHero";
import type { AppPage } from "../components/AppShell";
import { CleanupDialog } from "../components/CleanupDialog";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { DeployCelebration } from "../components/DeployCelebration";
import { Dropdown } from "../components/Dropdown";
import { EmptyState } from "../components/EmptyState";
import { Callout, PlanFailureNotice, PlanPreview } from "../components/PlanPreview";
import { PromptList } from "../components/PromptList";
import { QuickDeployPanel } from "../components/QuickDeployPanel";
import { ToolLogo } from "../components/ToolLogos";
import { ZCodeBanner } from "../components/ZCodeBanner";
import { IconAlert, IconPlus, IconSearch } from "../components/icons";
import { Button, Disclosure, Input, Mono, cx } from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import { applyHarnessOutcome, getHarnessStatus, loadHarnessStatus, useHarnessStatus } from "../lib/harnessState";
import { DEPLOY_PROMPT_EVENT, QUICK_DEPLOY_EVENT } from "../lib/paletteEvents";
import { canConfirmPlan } from "../lib/planGate";
import { failureFromEnvelope, needsReconcile, readableError } from "../lib/planFailure";
import type { PlanFailure } from "../lib/planFailure";
import { toastSafeMessage } from "../lib/redact";
import { activeIdsFor, isRecoveryState, mergeTools } from "../lib/tools";
import { isZcodeUnavailable } from "../lib/zcode";
import type {
  Activation,
  Operation,
  PlanResult,
  PromptSort,
  PromptSummary,
  Settings,
  ToolId,
  ToolInfo,
} from "../types";

type PlanKind = "activate" | "deactivate" | "recover" | "reconcile";

interface OpenPlan {
  kind: PlanKind;
  result: PlanResult;
  /** The library prompt being deployed; null for removals and recoveries. */
  promptId: string | null;
  title: string;
  /** Plans opened from the composer keep the draft dirty until they resolve. */
  source: "composer" | "library" | "machine";
  /** The deploy that was blocked and is planned again once this repair is done. */
  then?: { promptId: string; title: string; source: "composer" | "library" | "machine" };
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
 * and the Quick Deploy composer one click away. Every deployment is machine-wide
 * (user scope) and every write still goes through a reviewed adapter plan.
 *
 * The page is keyed by tool in the app, so switching agents starts from clean
 * state; the machine read itself is remembered per run in `harnessState`.
 */
export function WorkspacePage({
  tool,
  settings,
  toast,
  onNavigate,
  onDirtyChange,
  libraryEpoch = 0,
}: {
  tool: ToolId;
  settings?: Settings;
  toast?: ToastApi;
  onNavigate?: (page: AppPage) => void;
  onDirtyChange?: (dirty: boolean) => void;
  libraryEpoch?: number;
}) {
  const { t } = useTranslation();
  const entry = useHarnessStatus(tool);
  const defaultTitle = t("quickDeploy.defaultTitle");
  const [toolInfo, setToolInfo] = useState<ToolInfo>(() => mergeTools(null).find((item) => item.id === tool)!);
  const [reading, setReading] = useState(false);
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
  /** Set once a confirmed plan failed: that plan is spent, so only a new one can go ahead. */
  const [planFailure, setPlanFailure] = useState<PlanFailure | null>(null);
  const [busy, setBusy] = useState(false);
  /** Which action the person just started, so its own button shows progress. */
  const [pending, setPending] = useState<string | null>(null);
  const [celebration, setCelebration] = useState<{ kind: "deployed" | "removed"; subtitle: string } | null>(null);
  const [cleanupOpen, setCleanupOpen] = useState(false);
  const promptSeq = useRef(0);

  const unavailable = isZcodeUnavailable(toolInfo) || !toolInfo.available;

  const machine = entry?.error ? "unknown" : entry?.machine ?? "unknown";
  const statusError = loadError ?? entry?.error;

  // The activation record names the live prompt; the machine read backs it up
  // when that record is missing (deployed by an older version, rebuilt index).
  const livePromptId = machine === "deployed" ? entry?.promptId ?? null : null;
  const activeIds = useMemo(() => {
    if (!activations) return livePromptId ? [livePromptId] : null;
    const ids = activeIdsFor(activations, tool, "user", "");
    return livePromptId && !ids.includes(livePromptId) ? [...ids, livePromptId] : ids;
  }, [activations, livePromptId, tool]);
  const deployedTitle = useMemo(() => {
    if (!activeIds?.length) return entry?.promptTitle ?? null;
    const hit = prompts.find((item) => activeIds.includes(item.id));
    if (hit) return hit.title;
    const activation = activations?.find((item) => item.promptId && activeIds.includes(item.promptId));
    return activation?.promptTitle ?? entry?.promptTitle ?? null;
  }, [activations, activeIds, entry?.promptTitle, prompts]);

  // Deployed on the machine but absent from the library: the live group gets a
  // card of its own so the list never contradicts the hero.
  const unrecordedLive =
    machine === "deployed" &&
    !unavailable &&
    !promptsLoading &&
    !promptsError &&
    !prompts.some((item) => activeIds?.includes(item.id));

  const draftDirty =
    (title !== defaultTitle || Boolean(content)) && savedDraft !== `${title}\0${content}`;
  const dirty = draftDirty || plan?.source === "composer" || busy;

  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);

  useEffect(() => () => onDirtyChange?.(false), [onDirtyChange]);

  // Tool metadata.
  useEffect(() => {
    let cancelled = false;
    void api
      .listTools()
      .then(({ tools }) => {
        if (cancelled) return;
        const next = mergeTools(tools).find((item) => item.id === tool);
        if (!next) return;
        setToolInfo(next);
      })
      .catch((reason: unknown) => {
        if (!cancelled) setLoadError(toastSafeMessage(reason) || t("errors.loadFailed"));
      });
    return () => {
      cancelled = true;
    };
  }, [tool]);

  // Read the machine once per run.
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
      await loadHarnessStatus(tool, true);
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
    setBusy(true);
    setComposerMessage(null);
    try {
      const prompt = await api.createPastedPrompt({ tool, title: title.trim(), content });
      setSavedDraft(`${title}\0${content}`);
      const result = await api.planActivate({ promptId: prompt.id, scope: "user" });
      setPlanFailure(null);
      setPlan({ kind: "activate", result, promptId: prompt.id, title: prompt.title, source: "composer" });
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
    setBusy(true);
    setPending(`deploy:${promptId}`);
    try {
      const result = await api.planActivate({ promptId, scope: "user" });
      const summary = prompts.find((item) => item.id === promptId);
      setPlanFailure(null);
      setPlan({ kind: "activate", result, promptId, title: summary?.title ?? "", source: "library" });
    } catch (reason) {
      fail(reason, t("errors.planFailed"));
    } finally {
      setBusy(false);
      setPending(null);
    }
  };

  /**
   * Edit what the agent is running. A library prompt opens directly; a prompt the
   * library does not have is read back from the machine first, and when that is
   * not possible the composer opens so it can be pasted in.
   */
  const editLivePrompt = async () => {
    if (busy) return;
    const openEditor = (promptId: string) =>
      onNavigate?.({ kind: "prompt-edit", tool, promptId, creating: false, scope: "user", projectDir: "" });
    const libraryId = activeIds?.find((id) => prompts.some((item) => item.id === id));
    if (libraryId) {
      openEditor(libraryId);
      return;
    }
    setBusy(true);
    setPending("adopt");
    try {
      const detail = await api.adoptLivePrompt({ tool, title: t("prompts.adoptTitle", { tool: toolName }) });
      // The library now names what the machine runs; the hero must not wait for a re-read.
      applyHarnessOutcome(tool, "deploy", { id: detail.id, title: detail.title });
      openEditor(detail.id);
    } catch {
      toast?.err(t("prompts.adoptFailed"));
      setComposerMessage(null);
      setComposerOpen(true);
    } finally {
      setBusy(false);
      setPending(null);
    }
  };

  // The command palette asks this workspace to open the composer or a deploy sheet.
  const paletteRef = useRef({ openComposer, deployFromLibrary });
  paletteRef.current = { openComposer, deployFromLibrary };
  useEffect(() => {
    const onQuickDeploy = () => paletteRef.current.openComposer();
    const onDeployPrompt = (event: Event) => {
      const id = (event as CustomEvent<string>).detail;
      if (id) void paletteRef.current.deployFromLibrary(id);
    };
    window.addEventListener(QUICK_DEPLOY_EVENT, onQuickDeploy);
    window.addEventListener(DEPLOY_PROMPT_EVENT, onDeployPrompt);
    return () => {
      window.removeEventListener(QUICK_DEPLOY_EVENT, onQuickDeploy);
      window.removeEventListener(DEPLOY_PROMPT_EVENT, onDeployPrompt);
    };
  }, []);

  const openRemovePlan = async () => {
    if (busy) return;
    setBusy(true);
    setPending("remove");
    try {
      const result = await api.planDeactivate({ tool, scope: "user" });
      setPlanFailure(null);
      setPlan({ kind: "deactivate", result, promptId: null, title: deployedTitle ?? "", source: "machine" });
    } catch (reason) {
      fail(reason, t("quickDeploy.failed"));
    } finally {
      setBusy(false);
      setPending(null);
    }
  };

  const openRecoverPlan = async () => {
    if (busy) return;
    setBusy(true);
    setPending("recover");
    try {
      const result = await api.recoverTool({ tool, scope: "user" });
      setPlanFailure(null);
      setPlan({ kind: "recover", result, promptId: null, title: "", source: "machine" });
    } catch (reason) {
      fail(reason, t("errors.recoverFailed"));
    } finally {
      setBusy(false);
      setPending(null);
    }
  };

  const planBlocked = (candidate: OpenPlan | null) =>
    !candidate ||
    !canConfirmPlan(candidate.result.envelope) ||
    (candidate.kind !== "recover" && candidate.kind !== "reconcile" && isRecoveryState(candidate.result.envelope));

  /** Plan the same action again, in the open dialog, after a plan was spent or a repair was made. */
  const replan = async (from: OpenPlan | null = plan): Promise<boolean> => {
    if (!from) return false;
    setBusy(true);
    setPlanFailure(null);
    try {
      if (from.kind === "activate" && from.promptId) {
        const result = await api.planActivate({ promptId: from.promptId, scope: "user" });
        setPlan({ ...from, result });
      } else if (from.kind === "deactivate") {
        const result = await api.planDeactivate({ tool, scope: "user" });
        setPlan({ ...from, result });
      } else if (from.kind === "recover") {
        const result = await api.recoverTool({ tool, scope: "user" });
        setPlan({ ...from, result });
      } else if (from.kind === "reconcile") {
        const result = await api.planReconcile(tool);
        setPlan({ ...from, result });
      }
      return true;
    } catch (reason) {
      fail(reason, t("errors.planFailed"));
      return false;
    } finally {
      setBusy(false);
    }
  };

  /** Grok's config was changed behind Keysmith's back: tidy it (previewed first), then deploy again. */
  const openReconcilePlan = async () => {
    if (busy || !plan) return;
    setBusy(true);
    try {
      const result = await api.planReconcile(tool);
      setPlanFailure(null);
      setPlan({
        kind: "reconcile",
        result,
        promptId: null,
        title: "",
        source: "machine",
        then: { promptId: plan.promptId ?? "", title: plan.title, source: plan.source },
      });
    } catch (reason) {
      fail(reason, t("errors.planFailed"));
    } finally {
      setBusy(false);
    }
  };

  const confirmPlan = async () => {
    if (!plan || planBlocked(plan) || planFailure) return;
    setBusy(true);
    setPlanFailure(null);
    try {
      const result =
        plan.kind === "activate"
          ? await api.activate(plan.result.operationId)
          : plan.kind === "deactivate"
            ? await api.deactivate(plan.result.operationId)
            : plan.kind === "reconcile"
              ? await api.confirmReconcile(plan.result.operationId)
              : await api.confirmRecover(plan.result.operationId);
      if (!result.envelope.ok || result.envelope.exitCode !== 0) {
        const failure = failureFromEnvelope(result.envelope, t);
        setPlanFailure(failure);
        toast?.err(failure.message);
        return;
      }
      if (plan.kind === "reconcile") {
        toast?.ok(t("plan.reconciled"));
        // Back to what the person was doing: the deploy is planned again from the repaired state.
        if (plan.then?.promptId) {
          const next = await api.planActivate({ promptId: plan.then.promptId, scope: "user" });
          setPlan({ kind: "activate", result: next, promptId: plan.then.promptId, title: plan.then.title, source: plan.then.source });
        } else {
          setPlan(null);
        }
        setLocalEpoch((value) => value + 1);
        return;
      }
      if (plan.kind === "recover") void loadHarnessStatus(tool, true);
      else if (plan.kind === "activate") applyHarnessOutcome(tool, "deploy", { id: plan.promptId, title: plan.title });
      else applyHarnessOutcome(tool, "remove");
      if (plan.source === "composer") {
        resetDraft();
        setComposerOpen(false);
      }
      if (plan.kind === "activate") setCelebration({ kind: "deployed", subtitle: plan.title || t("quickDeploy.deployed") });
      else if (plan.kind === "deactivate") setCelebration({ kind: "removed", subtitle: plan.title });
      else toast?.ok(t("plan.success"));
      setPlan(null);
      setLocalEpoch((value) => value + 1);
    } catch (reason) {
      const message = readableError(reason, t);
      // Whatever went wrong, a plan that was confirmed once is not confirmed again.
      setPlanFailure({ message, detail: null });
      toast?.err(message);
    } finally {
      setBusy(false);
    }
  };

  const closePlan = () => {
    if (busy) return;
    setPlan(null);
    setPlanFailure(null);
  };

  const availableTags = useMemo(() => {
    const set = new Set<string>();
    for (const item of prompts) for (const value of item.tags) set.add(value);
    if (tag) set.add(tag);
    return [...set].sort();
  }, [prompts, tag]);

  const selectPrompt = (id: string) => {
    onNavigate?.({ kind: "prompt-view", tool, promptId: id, scope: "user", projectDir: "" });
  };

  const filtered = Boolean(query.trim() || tag);
  const locked = busy || Boolean(plan);
  const toolName = t(`nav.${tool}`);
  const planRecoverable = plan && plan.kind !== "recover" && plan.kind !== "reconcile" && isRecoveryState(plan.result.envelope);
  const planReconcilable = plan?.kind === "activate" && needsReconcile(tool, plan.result.envelope);
  // A deploy that cannot be confirmed can still be cleared away with the Cleanup dialog.
  const planCleanable =
    plan?.kind === "activate" && planBlocked(plan) && !unavailable;
  const openCleanupFromPlan = () => {
    if (busy) return;
    setPlan(null);
    setPlanFailure(null);
    setCleanupOpen(true);
  };

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
          statusError={statusError}
          deployedTitle={deployedTitle}
          unavailable={unavailable}
          busy={locked}
          removing={pending === "remove"}
          onRefresh={() => void refresh()}
          onRemove={() => void openRemovePlan()}
          removeDisabled={locked}
          onEdit={() => void editLivePrompt()}
          editing={pending === "adopt"}
          onCleanup={() => setCleanupOpen(true)}
        />

        <ZCodeBanner tool={toolInfo} />

        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <div className="relative min-w-[200px] flex-1">
              <IconSearch
                size={15}
                className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
              />
              <Input
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
                className="pl-9"
              />
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
            hint={toolInfo.unavailableReason ?? undefined}
            testId="prompt-list-unavailable"
          />
        ) : (
          <PromptList
            prompts={prompts}
            selectedId={null}
            activeIds={activeIds}
            loading={promptsLoading}
            filtered={filtered}
            unrecordedLive={unrecordedLive}
            onAdoptLive={() => void editLivePrompt()}
            adoptingLive={pending === "adopt"}
            onSelect={selectPrompt}
            onDeploy={(id) => void deployFromLibrary(id)}
            deployDisabled={locked}
            deployingId={pending?.startsWith("deploy:") ? pending.slice("deploy:".length) : null}
            engagedId={plan?.kind === "activate" ? plan.promptId : null}
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
        title={title}
        content={content}
        busy={busy}
        message={composerMessage}
        messageTone="error"
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

      <CleanupDialog
        tool={tool}
        toolName={toolName}
        open={cleanupOpen}
        onClose={() => setCleanupOpen(false)}
        onDone={(result) => {
          // The agent runs nothing of Keysmith's now: the hero, the rail and the list must say so.
          if (result.deactivated) applyHarnessOutcome(tool, "remove");
          void loadHarnessStatus(tool, true);
          setLocalEpoch((value) => value + 1);
          toast?.ok(result.snapshotId ? t("cleanup.done") : t("cleanup.doneNothing"));
        }}
        onOpenVersions={() => {
          setCleanupOpen(false);
          onNavigate?.({ kind: "settings", tab: "versions" });
        }}
      />

      <ConfirmDialog
        open={Boolean(plan)}
        wide
        icon={<ToolLogo tool={tool} size={22} />}
        title={
          plan?.kind === "deactivate"
            ? t("plan.titleDeactivate", { tool: toolName })
            : plan?.kind === "recover"
              ? t("operations.recover")
              : plan?.kind === "reconcile"
                ? t("plan.titleReconcile", { tool: toolName })
                : t("plan.titleDeploy", { tool: toolName })
        }
        description={[toolName, plan?.title].filter(Boolean).join(" · ")}
        confirmLabel={
          plan?.kind === "deactivate"
            ? t("plan.confirmDeactivate")
            : plan?.kind === "recover"
              ? t("operations.restoreEntry")
              : plan?.kind === "reconcile"
                ? t("plan.confirmReconcile")
                : t("quickDeploy.confirm")
        }
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        busy={busy}
        danger={plan?.kind === "deactivate"}
        confirmDisabled={planBlocked(plan) || busy || Boolean(planFailure)}
        confirmTestId="quick-deploy-confirm"
        onClose={closePlan}
        onConfirm={() => void confirmPlan()}
        footerStart={
          planReconcilable ? (
            <Button size="sm" variant="outline" disabled={busy} data-testid="plan-reconcile" onClick={() => void openReconcilePlan()}>
              {t("plan.reconcileButton")}
            </Button>
          ) : planRecoverable || planCleanable ? (
            <div className="flex items-center gap-2">
              {planRecoverable ? (
                <Button size="sm" variant="ghost" disabled={busy} data-testid="plan-recover" onClick={() => void openRecoverPlan()}>
                  {t("operations.restoreEntry")}
                </Button>
              ) : null}
              {planCleanable ? (
                <Button size="sm" variant="ghost" disabled={busy} data-testid="plan-cleanup" onClick={openCleanupFromPlan}>
                  {t("cleanup.button")}
                </Button>
              ) : null}
            </div>
          ) : null
        }
      >
        {plan ? (
          <PlanPreview
            envelope={plan.result.envelope}
            tool={tool}
            kind={plan.kind}
            fromTitle={deployedTitle}
            toTitle={plan.title}
          />
        ) : null}
        {planFailure ? <PlanFailureNotice failure={planFailure} busy={busy} onReplan={() => void replan()} /> : null}
      </ConfirmDialog>

      {celebration ? (
        <DeployCelebration
          tool={tool}
          variant={celebration.kind}
          title={
            celebration.kind === "deployed"
              ? t("quickDeploy.deployedTo", { tool: toolName })
              : t("quickDeploy.removedFrom", { tool: toolName })
          }
          subtitle={celebration.subtitle}
          onDone={() => setCelebration(null)}
        />
      ) : null}
    </section>
  );
}
