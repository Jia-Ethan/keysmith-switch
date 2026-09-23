import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { HarnessAction, useHarnessPhase } from "../components/HarnessAction";
import { ToolLogo } from "../components/ToolLogos";
import type { ToolId } from "../types";

type Pending = "deploy" | "remove" | null;

export function HarnessPage({
  tool,
  onDirtyChange,
}: {
  tool: ToolId;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const { t } = useTranslation();
  const deploy = useHarnessPhase();
  const remove = useHarnessPhase();
  const [pending, setPending] = useState<Pending>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    onDirtyChange?.(busy || pending !== null);
  }, [busy, onDirtyChange, pending]);

  const run = async (action: Exclude<Pending, null>) => {
    const slot = action === "deploy" ? deploy : remove;
    slot.begin();
    setBusy(true);
    try {
      const outcome =
        action === "deploy" ? await api.deployHarness(tool) : await api.removeHarness(tool);
      if (outcome.ok) slot.succeed();
      else slot.fail(outcome.error || t("harness.genericError"));
    } catch (error) {
      const message = error instanceof Error ? error.message : t("harness.genericError");
      slot.fail(message || t("harness.genericError"));
    } finally {
      setBusy(false);
    }
  };

  const confirm = () => {
    const action = pending;
    setPending(null);
    if (action) void run(action);
  };

  return (
    <section data-testid="harness-page" className="mx-auto flex w-full max-w-xl flex-col gap-6 px-4 py-8">
      <header className="flex items-center gap-3">
        <ToolLogo tool={tool} size={28} />
        <div className="min-w-0">
          <h1 className="text-[15px] font-semibold text-foreground">{t(`nav.${tool}`)}</h1>
          <p className="text-[13px] leading-5 text-muted-foreground">{t("harness.lead")}</p>
        </div>
      </header>
      <div className="flex flex-col gap-3">
        <HarnessAction
          testId="harness-deploy"
          reasonTestId="harness-deploy-reason"
          label={t("harness.deploy")}
          busyLabel={t("harness.busyDeploy")}
          successLabel={t("harness.done")}
          retryLabel={t("harness.retry")}
          phase={deploy.phase}
          reason={deploy.reason}
          onRun={() => setPending("deploy")}
        />
        <HarnessAction
          testId="harness-remove"
          reasonTestId="harness-remove-reason"
          label={t("harness.remove")}
          busyLabel={t("harness.busyRemove")}
          successLabel={t("harness.done")}
          retryLabel={t("harness.retry")}
          phase={remove.phase}
          reason={remove.reason}
          danger
          onRun={() => setPending("remove")}
        />
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
