import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import type { ToastApi } from "../hooks/useToasts";
import { canConfirmPlan } from "../lib/planGate";
import { toastSafeMessage } from "../lib/redact";
import { activeIdsFor, isRecoveryState, scopeNeedsProjectDir } from "../lib/tools";
import type {
  Activation,
  Envelope,
  PlanResult,
  PromptDetail,
  ScopeId,
  ToolId,
} from "../types";
import { ConfirmDialog } from "./ConfirmDialog";
import { ErrorBanner } from "./ErrorBanner";
import { PlanPreview } from "./PlanPreview";
import { PromptViewPage } from "./PromptViewPage";
import { Button } from "./ui";

export function PromptDetailPage({
  promptId,
  tool,
  scope,
  projectDir,
  toast,
  onClose,
  onEdit,
  onChanged,
}: {
  promptId: string;
  tool: ToolId;
  scope: ScopeId;
  projectDir: string;
  toast: ToastApi;
  onClose: () => void;
  onEdit: (detail: PromptDetail) => void;
  onChanged: () => void;
}) {
  const { t } = useTranslation();
  const [activations, setActivations] = useState<Activation[] | null>(null);
  const [status, setStatus] = useState<Envelope | null>(null);
  const [contextLoading, setContextLoading] = useState(true);
  const [contextError, setContextError] = useState(false);
  const [busy, setBusy] = useState(false);
  const [refreshEpoch, setRefreshEpoch] = useState(0);
  const [plan, setPlan] = useState<{ kind: "activate" | "deactivate"; result: PlanResult } | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const currentDetailRef = useRef<PromptDetail | null>(null);
  const contextSeq = useRef(0);
  const requireProject = scopeNeedsProjectDir(scope);
  const projectReady = !requireProject || Boolean(projectDir.trim());

  const loadContext = useCallback(async () => {
    const seq = ++contextSeq.current;
    setContextLoading(true);
    setContextError(false);
    const [activationResult, statusResult] = await Promise.allSettled([
      api.listActivations(tool),
      api.toolStatus({
        tool,
        scope,
        projectDir: requireProject && projectDir ? projectDir : undefined,
      }),
    ]);
    if (seq !== contextSeq.current) return;
    setActivations(
      activationResult.status === "fulfilled" ? activationResult.value.activations ?? [] : null,
    );
    setStatus(statusResult.status === "fulfilled" ? statusResult.value : null);
    setContextError(activationResult.status === "rejected" || statusResult.status === "rejected");
    setContextLoading(false);
  }, [projectDir, requireProject, scope, tool]);

  useEffect(() => {
    void loadContext();
    return () => {
      contextSeq.current += 1;
    };
  }, [loadContext]);

  const activeIds = useMemo(
    () => (activations ? activeIdsFor(activations, tool, scope, projectDir) : null),
    [activations, projectDir, scope, tool],
  );
  const isActiveHere = activeIds ? activeIds.includes(promptId) : null;
  const contextReady = !contextLoading && !contextError && status?.available === true && activeIds !== null;
  const unavailable = !contextReady;

  const openPlan = async (kind: "activate" | "deactivate") => {
    const detail = currentDetailRef.current;
    if (busy || unavailable) {
      if (contextError) toast.err(t("errors.loadFailed"));
      return;
    }
    if (activeIds === null) {
      toast.err(t("prompts.activationUnknown"));
      return;
    }
    if (!projectReady) {
      toast.err(t("scope.needsProjectDir"));
      return;
    }
    if (!detail) {
      toast.err(t("errors.validation"));
      return;
    }
    setBusy(true);
    setPlanError(null);
    try {
      const result =
        kind === "activate"
          ? await api.planActivate({
              promptId: detail.id,
              scope,
              projectDir: requireProject ? projectDir : undefined,
            })
          : await api.planDeactivate({
              promptId: detail.id,
              tool,
              scope,
              projectDir: requireProject ? projectDir : undefined,
            });
      setPlan({ kind, result });
    } catch (err) {
      setPlan(null);
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  const confirmPlan = async () => {
    if (!plan || !canConfirmPlan(plan.result.envelope) || isRecoveryState(plan.result.envelope)) return;
    setBusy(true);
    setPlanError(null);
    try {
      const result =
        plan.kind === "activate"
          ? await api.activate(plan.result.operationId)
          : await api.deactivate(plan.result.operationId);
      if (!result.envelope.ok || result.envelope.exitCode !== 0) {
        const reason = toastSafeMessage(result.envelope.error || t("plan.failed"));
        setPlanError(reason);
        toast.err(reason);
        return;
      }
      toast.ok(t("plan.success"));
      setPlan(null);
      await loadContext();
      onChanged();
    } catch (err) {
      const reason = toastSafeMessage(err);
      setPlanError(reason);
      toast.err(reason);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <PromptViewPage
        key={`${promptId}:${refreshEpoch}`}
        promptId={promptId}
        tool={tool}
        isActiveHere={isActiveHere}
        disabled={unavailable || !projectReady}
        busy={busy}
        toast={toast}
        onClose={onClose}
        onLoaded={(detail) => {
          currentDetailRef.current = detail;
        }}
        onEdit={onEdit}
        onActivate={() => void openPlan("activate")}
        onDeactivate={() => void openPlan("deactivate")}
        onChanged={() => {
          setRefreshEpoch((value) => value + 1);
          onChanged();
        }}
        onDeleted={onClose}
      />
      {contextError ? (
        <div className="fixed bottom-5 left-1/2 z-[65] w-[min(440px,calc(100vw-2rem))] -translate-x-1/2 shadow-lg">
          <ErrorBanner
            message={t("errors.loadFailed")}
            retryLabel={t("common.retry")}
            onRetry={() => void loadContext()}
          />
        </div>
      ) : null}
      <ConfirmDialog
        open={Boolean(plan)}
        wide
        title={plan?.kind === "deactivate" ? t("plan.titleDeactivate") : t("plan.titleActivate")}
        description={currentDetailRef.current?.title}
        confirmLabel={plan?.kind === "deactivate" ? t("plan.confirmDeactivate") : t("plan.confirmActivate")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        busy={busy}
        confirmDisabled={!plan || !canConfirmPlan(plan.result.envelope) || isRecoveryState(plan.result.envelope) || busy}
        confirmTestId="plan-confirm"
        onClose={() => {
          if (busy) return;
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
            <Button className="mt-2" size="sm" onClick={() => void loadContext()}>
              {t("common.retry")}
            </Button>
          </div>
        ) : null}
      </ConfirmDialog>
    </>
  );
}
