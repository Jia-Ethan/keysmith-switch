import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { HarnessAction, useHarnessPhase } from "../components/HarnessAction";
import { ToolLogo } from "../components/ToolLogos";
import type { ToolId } from "../types";

type Machine = "unknown" | "deployed" | "undeployed";
type Pending = "deploy" | "remove" | null;

export function HarnessPage({
  tool,
  onDirtyChange,
}: {
  tool: ToolId;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const { t } = useTranslation();
  const action = useHarnessPhase();
  const [machine, setMachine] = useState<Machine>("unknown");
  const [statusError, setStatusError] = useState<string | null>(null);
  const [pending, setPending] = useState<Pending>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    onDirtyChange?.(busy || pending !== null);
  }, [busy, onDirtyChange, pending]);

  useEffect(() => {
    let cancelled = false;
    setMachine("unknown");
    setStatusError(null);
    void api
      .getHarnessState(tool)
      .then((state) => {
        if (cancelled) return;
        setMachine(state.deployed ? "deployed" : "undeployed");
        setStatusError(state.error);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setMachine("undeployed");
        setStatusError(error instanceof Error ? error.message : t("harness.genericError"));
      });
    return () => {
      cancelled = true;
    };
  }, [t, tool]);

  const run = async (next: Exclude<Pending, null>) => {
    action.begin();
    setBusy(true);
    try {
      const outcome = next === "deploy" ? await api.deployHarness(tool) : await api.removeHarness(tool);
      if (outcome.ok) {
        setMachine(next === "deploy" ? "deployed" : "undeployed");
        setStatusError(null);
        action.succeed();
      } else {
        action.fail(outcome.error || t("harness.genericError"));
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : t("harness.genericError");
      action.fail(message || t("harness.genericError"));
    } finally {
      setBusy(false);
    }
  };

  const confirm = () => {
    const next = pending;
    setPending(null);
    if (next) void run(next);
  };

  const failed = action.phase === "failure";
  const removing = machine === "deployed";
  const statusText = failed
    ? action.reason
    : statusError
      ? statusError
      : machine === "unknown"
        ? t("harness.reading")
        : removing
          ? t("harness.deployed")
          : t("harness.undeployed");

  return (
    <section
      data-testid="harness-page"
      data-machine={machine}
      className="flex h-full min-h-0 w-full items-start justify-center overflow-auto px-6 py-10"
    >
      <div className="flex w-full max-w-2xl flex-col gap-5 rounded-2xl border border-border bg-card px-6 py-6 shadow-[0_16px_50px_hsl(var(--foreground)/0.04)]">
        <header className="flex items-center gap-3">
          <ToolLogo tool={tool} size={32} />
          <div className="min-w-0">
            <h1 className="text-[17px] font-semibold text-foreground">{t(`nav.${tool}`)}</h1>
            <p className="text-[13px] leading-5 text-muted-foreground">{t("harness.lead")}</p>
          </div>
        </header>
        <div className="flex min-h-8 flex-wrap items-center gap-3">
          {machine === "unknown" && !failed ? (
            <p data-testid="harness-status" className="text-[13px] leading-5 text-muted-foreground">
              {statusText}
            </p>
          ) : (
            <HarnessAction
              testId={removing ? "harness-remove" : "harness-deploy"}
              reasonTestId="harness-reason"
              label={removing ? t("harness.remove") : t("harness.deploy")}
              busyLabel={removing ? t("harness.busyRemove") : t("harness.busyDeploy")}
              successLabel={t("harness.done")}
              retryLabel={t("harness.retry")}
              phase={action.phase}
              reason={null}
              danger={removing}
              onRun={() => setPending(removing ? "remove" : "deploy")}
            />
          )}
          {machine !== "unknown" || failed ? (
            <p
              data-testid="harness-status"
              className={
                failed || statusError
                  ? "min-w-0 text-[13px] leading-5 text-destructive"
                  : "min-w-0 text-[13px] leading-5 text-muted-foreground"
              }
            >
              {statusText}
            </p>
          ) : null}
        </div>
      </div>
      <ConfirmDialog
        open={pending !== null}
        title={pending === "remove" ? t("harness.confirmRemoveTitle") : t("harness.confirmDeployTitle")}
        confirmLabel={t("common.confirm")}
        cancelLabel={t("common.cancel")}
        confirmTestId="harness-confirm"
        danger={pending === "remove"}
        busy={busy}
        onConfirm={confirm}
        onClose={() => {
          if (!busy) setPending(null);
        }}
      >
        <p className="text-[13px] leading-5 text-muted-foreground">
          {pending === "remove" ? t("harness.confirmRemoveBody") : t("harness.confirmDeployBody")}
        </p>
      </ConfirmDialog>
    </section>
  );
}
