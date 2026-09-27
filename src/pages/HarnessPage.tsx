import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { AppPage } from "../components/AppShell";
import { MarkdownEditor } from "../components/MarkdownEditor";
import { PlanPreview } from "../components/PlanPreview";
import { ScopeBar } from "../components/ScopeBar";
import { ToolLogo } from "../components/ToolLogos";
import { IconRefresh } from "../components/icons";
import { Button, Field, IconButton, Input } from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import { applyHarnessOutcome, getHarnessStatus, loadHarnessStatus, useHarnessStatus } from "../lib/harnessState";
import { canConfirmPlan } from "../lib/planGate";
import { toastSafeMessage } from "../lib/redact";
import { pickDirectory } from "../lib/runtime";
import { defaultScopeFor, isRecoveryState, mergeTools, scopeNeedsProjectDir } from "../lib/tools";
import type { Envelope, PlanResult, ScopeId, Settings, ToolId, ToolInfo } from "../types";

export function HarnessPage({
  tool,
  settings,
  toast,
  onRememberProject,
  onNavigate,
  onDirtyChange,
}: {
  tool: ToolId;
  settings?: Settings;
  toast?: ToastApi;
  onRememberProject?: (dir: string) => void;
  onNavigate?: (page: AppPage) => void;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const { t } = useTranslation();
  const entry = useHarnessStatus(tool);
  const [toolInfo, setToolInfo] = useState<ToolInfo>(() => mergeTools(null).find((item) => item.id === tool)!);
  const [status, setStatus] = useState<Envelope | null>(null);
  const [title, setTitle] = useState(() => t("quickDeploy.defaultTitle"));
  const [content, setContent] = useState("");
  const [savedDraft, setSavedDraft] = useState<string | null>(null);
  const [scope, setScope] = useState<ScopeId>("user");
  const [projectDir, setProjectDir] = useState("");
  const [plan, setPlan] = useState<{
    kind: "activate" | "deactivate";
    result: PlanResult;
    scope: ScopeId;
    projectDir: string;
    title: string;
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const [reading, setReading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusEpoch, setStatusEpoch] = useState(0);
  const [projectStatusError, setProjectStatusError] = useState<string | null>(null);

  const supportedScopes = useMemo(() => toolInfo.supportedScopes.length ? toolInfo.supportedScopes : ["user"] as ScopeId[], [toolInfo]);
  const requireProject = scopeNeedsProjectDir(scope);
  const projectReady = !requireProject || Boolean(projectDir.trim());
  const machine = scope === "user"
    ? entry?.error ? "unknown" : entry?.machine ?? "unknown"
    : status && status.available
      ? status.status === "active" ? "deployed" : status.status === "inactive" || status.status === "not-installed" ? "undeployed" : "unknown"
      : "unknown";
  const statusError = error ?? (scope === "user" ? entry?.error : projectStatusError ?? status?.error);
  const dirty = ((title !== t("quickDeploy.defaultTitle") || Boolean(content)) && savedDraft !== `${title}\0${content}`) || Boolean(plan) || busy;

  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);

  useEffect(() => () => onDirtyChange?.(false), [onDirtyChange]);

  useEffect(() => {
    setTitle(t("quickDeploy.defaultTitle"));
    setContent("");
    setSavedDraft(null);
    setProjectDir("");
    setPlan(null);
    setStatus(null);
    setError(null);
    setProjectStatusError(null);
    setToolInfo(mergeTools(null).find((item) => item.id === tool)!);
    setScope("user");
    let cancelled = false;
    void api.listTools()
      .then(({ tools }) => {
        if (cancelled) return;
        const next = tools.find((item) => item.id === tool);
        if (!next) return;
        setToolInfo(next);
        setScope(defaultScopeFor(tool, next.supportedScopes, settings?.defaultClaudeScope ?? "user"));
      })
      .catch((reason: unknown) => {
        if (!cancelled) setError(toastSafeMessage(reason) || t("errors.loadFailed"));
      });
    return () => { cancelled = true; };
  }, [settings?.defaultClaudeScope, t, tool]);

  useEffect(() => {
    if (getHarnessStatus(tool)) return;
    let cancelled = false;
    setReading(true);
    void loadHarnessStatus(tool).finally(() => {
      if (!cancelled) setReading(false);
    });
    return () => { cancelled = true; };
  }, [tool]);

  useEffect(() => {
    setStatus(null);
    setProjectStatusError(null);
    if (!requireProject || !projectDir.trim()) return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void api.toolStatus({ tool, scope, projectDir: projectDir.trim() })
        .then((result) => {
          if (!cancelled) setStatus(result);
        })
        .catch((reason: unknown) => {
          if (!cancelled) setProjectStatusError(toastSafeMessage(reason) || t("errors.loadFailed"));
        });
    }, 200);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [projectDir, requireProject, scope, statusEpoch, t, tool]);

  const refresh = async () => {
    setReading(true);
    setError(null);
    try {
      if (scope === "user") await loadHarnessStatus(tool, true);
      else setStatusEpoch((value) => value + 1);
    } finally {
      setReading(false);
    }
  };

  const openActivatePlan = async () => {
    if (!title.trim() || !content.trim()) {
      const reason = t("quickDeploy.validation");
      setError(reason);
      toast?.err(reason);
      return;
    }
    if (!projectReady) {
      const reason = t("scope.needsProjectDir");
      setError(reason);
      toast?.err(reason);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const plannedScope = scope;
      const plannedDir = requireProject ? projectDir.trim() : "";
      const prompt = await api.createPastedPrompt({ tool, title: title.trim(), content });
      setSavedDraft(`${title}\0${content}`);
      const result = await api.planActivate({
        promptId: prompt.id,
        scope: plannedScope,
        projectDir: plannedDir || undefined,
      });
      setPlan({ kind: "activate", result, scope: plannedScope, projectDir: plannedDir, title: prompt.title });
    } catch (reason) {
      const message = toastSafeMessage(reason) || t("quickDeploy.failed");
      setError(message);
      toast?.err(message);
    } finally {
      setBusy(false);
    }
  };

  const openRemovePlan = async () => {
    if (!projectReady) return;
    setBusy(true);
    setError(null);
    try {
      const plannedDir = requireProject ? projectDir.trim() : "";
      const result = await api.planDeactivate({
        tool,
        scope,
        projectDir: plannedDir || undefined,
      });
      setPlan({ kind: "deactivate", result, scope, projectDir: plannedDir, title: "" });
    } catch (reason) {
      const message = toastSafeMessage(reason) || t("quickDeploy.failed");
      setError(message);
      toast?.err(message);
    } finally {
      setBusy(false);
    }
  };

  const confirmPlan = async () => {
    if (!plan || !canConfirmPlan(plan.result.envelope) || isRecoveryState(plan.result.envelope)) return;
    setBusy(true);
    setError(null);
    try {
      const result = plan.kind === "activate"
        ? await api.activate(plan.result.operationId)
        : await api.deactivate(plan.result.operationId);
      if (!result.envelope.ok || result.envelope.exitCode !== 0) {
        const message = toastSafeMessage(result.envelope.error || t("quickDeploy.failed"));
        setError(message);
        toast?.err(message);
        return;
      }
      if (plan.scope === "user") applyHarnessOutcome(tool, plan.kind === "activate" ? "deploy" : "remove");
      else setStatusEpoch((value) => value + 1);
      setPlan(null);
      if (plan.kind === "activate") toast?.ok(t("quickDeploy.deployed"));
    } catch (reason) {
      const message = toastSafeMessage(reason) || t("quickDeploy.failed");
      setError(message);
      toast?.err(message);
    } finally {
      setBusy(false);
    }
  };

  const disabled = busy || Boolean(plan);
  return (
    <section data-testid="harness-page" data-machine={machine} className="flex h-full min-h-0 w-full items-start justify-center overflow-auto px-6 py-8">
      <div className="flex w-full max-w-4xl flex-col gap-5 rounded-2xl border border-border bg-card px-6 py-6 shadow-[0_1px_3px_hsl(var(--shadow)/0.04)]">
        <header className="flex items-center gap-3">
          <ToolLogo tool={tool} size={34} />
          <div className="min-w-0 flex-1">
            <h1 className="text-[19px] font-semibold tracking-[-0.01em] text-foreground">{t("quickDeploy.title")}</h1>
            <p className="mt-0.5 text-[13px] leading-5 text-muted-foreground">{t("quickDeploy.lead", { tool: t(`nav.${tool}`) })}</p>
          </div>
          <IconButton label={t("quickDeploy.refresh")} data-testid="harness-refresh" disabled={reading || busy} onClick={() => void refresh()}>
            <IconRefresh className={reading ? "harness-spin" : undefined} />
          </IconButton>
        </header>

        <ScopeBar
          scope={scope}
          supportedScopes={supportedScopes}
          projectDir={projectDir}
          recentProjectDirs={settings?.recentProjectDirs ?? []}
          disabled={disabled}
          onScopeChange={(next) => { setScope(next); setError(null); }}
          onProjectDirChange={(dir) => { setProjectDir(dir); setError(null); }}
          onBrowse={() => {
            void pickDirectory().then((dir) => {
              if (dir) { setProjectDir(dir); onRememberProject?.(dir); }
            });
          }}
        />

        <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
          <Field label={t("quickDeploy.titleLabel")} hint={t("quickDeploy.titleHint")}>
            <Input value={title} disabled={disabled} placeholder={t("quickDeploy.titlePlaceholder")} data-testid="quick-deploy-title" onChange={(event) => setTitle(event.target.value)} />
          </Field>
          <div className="min-w-0">
            <div className="mb-2 text-[14px] font-medium text-muted-foreground">{t("quickDeploy.contentLabel")}</div>
            <MarkdownEditor value={content} onChange={setContent} placeholder={t("quickDeploy.contentPlaceholder")} ariaLabel={t("quickDeploy.contentLabel")} readOnly={disabled} minHeight="300px" />
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-3 border-t border-border pt-4">
          <Button variant="primary" disabled={disabled || !toolInfo.available} data-testid="quick-deploy-submit" onClick={() => void openActivatePlan()}>
            {busy && !plan ? t("common.busy") : t("quickDeploy.preview")}
          </Button>
          {machine === "deployed" ? (
            <Button variant="danger" disabled={disabled || !projectReady} data-testid="quick-deploy-remove" onClick={() => void openRemovePlan()}>
              {t("quickDeploy.remove")}
            </Button>
          ) : null}
          {onNavigate ? (
            <Button variant="outline" disabled={disabled} data-testid="quick-deploy-library" onClick={() => onNavigate({ kind: "tool", tool })}>
              {t("prompts.library")}
            </Button>
          ) : null}
          <span data-testid="quick-deploy-status" data-reading={reading || undefined} className={statusError ? "text-[13px] text-destructive" : "text-[13px] text-muted-foreground"}>
            {statusError ?? (machine === "unknown" ? t("quickDeploy.reading") : machine === "deployed" ? t("quickDeploy.deployedState") : t("quickDeploy.undeployedState"))}
          </span>
        </div>
      </div>

      <ConfirmDialog
        open={Boolean(plan)}
        wide
        title={plan?.kind === "deactivate" ? t("plan.titleDeactivate") : t("quickDeploy.previewTitle")}
        description={[t(`nav.${tool}`), t(`scope.${plan?.scope ?? scope}`), plan?.projectDir, plan?.title].filter(Boolean).join(" · ")}
        confirmLabel={plan?.kind === "deactivate" ? t("plan.confirmDeactivate") : t("quickDeploy.confirm")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        busy={busy}
        confirmDisabled={!plan || !canConfirmPlan(plan.result.envelope) || isRecoveryState(plan.result.envelope) || busy}
        confirmTestId="quick-deploy-confirm"
        onClose={() => { if (!busy) setPlan(null); }}
        onConfirm={() => void confirmPlan()}
      >
        {plan ? <PlanPreview envelope={plan.result.envelope} /> : null}
      </ConfirmDialog>
    </section>
  );
}
