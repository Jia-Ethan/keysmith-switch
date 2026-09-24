import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { HarnessAction, useHarnessPhase } from "../components/HarnessAction";
import { ToolLogo } from "../components/ToolLogos";
import { IconButton } from "../components/ui";
import { IconRefresh } from "../components/icons";
import {
  applyHarnessOutcome,
  loadHarnessStatus,
  useHarnessStatus,
} from "../lib/harnessState";
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
  const entry = useHarnessStatus(tool);
  const [pending, setPending] = useState<Pending>(null);
  const [busy, setBusy] = useState(false);
  const [reading, setReading] = useState(false);

  const machine: Machine = entry ? entry.machine : "unknown";
  const statusError = entry?.error ?? null;

  useEffect(() => {
    onDirtyChange?.(busy || pending !== null);
  }, [busy, onDirtyChange, pending]);

  // Reads the machine once per run per tool. Switching tools and coming back
  // reuses the remembered result instead of reading again.
  useEffect(() => {
    let cancelled = false;
    if (entry) return undefined;
    setReading(true);
    void loadHarnessStatus(tool).finally(() => {
      if (!cancelled) setReading(false);
    });
    return () => {
      cancelled = true;
    };
  }, [entry, tool]);

  const refresh = async () => {
    setReading(true);
    try {
      await loadHarnessStatus(tool, true);
    } finally {
      setReading(false);
    }
  };

  const run = async (next: Exclude<Pending, null>) => {
    action.begin();
    setBusy(true);
    try {
      const outcome = next === "deploy" ? await api.deployHarness(tool) : await api.removeHarness(tool);
      if (outcome.ok) {
        applyHarnessOutcome(tool, next);
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
      className="flex h-full min-h-0 w-full items-start justify-center overflow-auto px-6 py-12"
    >
      <div className="flex w-full max-w-xl flex-col rounded-2xl border border-border bg-card shadow-[0_1px_3px_hsl(var(--shadow)/0.04)]">
        <header className="flex items-start gap-3.5 px-6 pt-6">
          <ToolLogo tool={tool} size={34} />
          <div className="min-w-0 flex-1">
            <h1 className="text-[19px] font-semibold tracking-[-0.01em] text-foreground">
              {t(`nav.${tool}`)}
            </h1>
            <p className="mt-0.5 text-[13px] leading-5 text-muted-foreground">{t("harness.lead")}</p>
          </div>
          <IconButton
            label={t("harness.refresh")}
            title={t("harness.refresh")}
            data-testid="harness-refresh"
            disabled={reading || busy}
            onClick={() => void refresh()}
          >
            <IconRefresh className={reading ? "harness-spin" : undefined} />
          </IconButton>
        </header>

        <div className="flex min-h-[3.25rem] flex-wrap items-center gap-3 px-6 pb-6 pt-5">
          {machine === "unknown" && !failed ? (
            <p
              data-testid="harness-status"
              data-reading={reading || undefined}
              className="inline-flex h-8 items-center gap-2 text-[13px] leading-5 text-muted-foreground"
            >
              <span className="harness-spinner" aria-hidden="true" />
              {statusText}
            </p>
          ) : (
            <>
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
            </>
          )}
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
