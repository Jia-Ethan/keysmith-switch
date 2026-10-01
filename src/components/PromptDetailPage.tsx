import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import type { ToastApi } from "../hooks/useToasts";
import { failureFromEnvelope, needsReconcile, readableError } from "../lib/planFailure";
import type { PlanFailure } from "../lib/planFailure";
import { canConfirmPlan } from "../lib/planGate";
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
import { PlanFailureNotice, PlanPreview } from "./PlanPreview";
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
  const [plan, setPlan] = useState<{ kind: "activate" | "deactivate" | "reconcile"; result: PlanResult; then?: "activate" } | null>(null);
  /** Set once a confirmed plan failed: that plan is spent, so only a new one can go ahead. */
  const [planFailure, setPlanFailure] = useState<PlanFailure | null>(null);
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
    setPlanFailure(null);
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

  /** Grok's config was changed behind Keysmith's back: tidy it (previewed first), then plan the deploy again. */
  const openReconcile = async () => {
    if (busy || !plan) return;
    setBusy(true);
    try {
      const result = await api.planReconcile(tool);
      setPlanFailure(null);
      setPlan({ kind: "reconcile", result, then: plan.kind === "activate" ? "activate" : plan.then });
    } catch (err) {
      toast.err(readableError(err, t));
    } finally {
      setBusy(false);
    }
  };

  const confirmPlan = async () => {
    if (!plan || planFailure) return;
    if (!canConfirmPlan(plan.result.envelope)) return;
    if (plan.kind !== "reconcile" && isRecoveryState(plan.result.envelope)) return;
    setBusy(true);
    setPlanFailure(null);
    try {
      const result =
        plan.kind === "activate"
          ? await api.activate(plan.result.operationId)
          : plan.kind === "reconcile"
            ? await api.confirmReconcile(plan.result.operationId)
            : await api.deactivate(plan.result.operationId);
      if (!result.envelope.ok || result.envelope.exitCode !== 0) {
        const failure = failureFromEnvelope(result.envelope, t);
        setPlanFailure(failure);
        toast.err(failure.message);
        return;
      }
      if (plan.kind === "reconcile") {
        toast.ok(t("plan.reconciled"));
        setPlan(null);
        await loadContext();
        // Back to what the person was doing: the deploy is planned again from the repaired state.
        if (plan.then === "activate") await openPlan("activate");
        return;
      }
      toast.ok(t("plan.success"));
      setPlan(null);
      await loadContext();
      onChanged();
    } catch (err) {
      // Whatever went wrong, a plan that was confirmed once is not confirmed again.
      const reason = readableError(err, t);
      setPlanFailure({ message: reason, detail: null });
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
        title={
          plan?.kind === "deactivate"
            ? t("plan.titleDeactivate", { tool: t(`nav.${tool}`) })
            : plan?.kind === "reconcile"
              ? t("plan.titleReconcile", { tool: t(`nav.${tool}`) })
              : t("plan.titleActivate")
        }
        description={currentDetailRef.current?.title}
        confirmLabel={
          plan?.kind === "deactivate"
            ? t("plan.confirmDeactivate")
            : plan?.kind === "reconcile"
              ? t("plan.confirmReconcile")
              : t("plan.confirmActivate")
        }
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        busy={busy}
        confirmDisabled={
          !plan ||
          !canConfirmPlan(plan.result.envelope) ||
          (plan.kind !== "reconcile" && isRecoveryState(plan.result.envelope)) ||
          busy ||
          Boolean(planFailure)
        }
        confirmTestId="plan-confirm"
        onClose={() => {
          if (busy) return;
          setPlan(null);
          setPlanFailure(null);
        }}
        onConfirm={() => void confirmPlan()}
        footerStart={
          plan?.kind === "activate" && needsReconcile(tool, plan.result.envelope) ? (
            <Button size="sm" variant="outline" disabled={busy} data-testid="plan-reconcile" onClick={() => void openReconcile()}>
              {t("plan.reconcileButton")}
            </Button>
          ) : null
        }
      >
        {plan ? (
          <PlanPreview
            envelope={plan.result.envelope}
            tool={tool}
            kind={plan.kind}
            fromTitle={plan.kind === "deactivate" ? currentDetailRef.current?.title : null}
            toTitle={currentDetailRef.current?.title}
          />
        ) : null}
        {planFailure && plan ? (
          <PlanFailureNotice
            failure={planFailure}
            busy={busy}
            onReplan={() => (plan.kind === "reconcile" ? void openReconcile() : void openPlan(plan.kind))}
          />
        ) : null}
      </ConfirmDialog>
    </>
  );
}
