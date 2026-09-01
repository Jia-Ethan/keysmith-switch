import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ErrorBanner } from "../components/ErrorBanner";
import { Button, Field, Input, Panel, PanelHeader, Segmented } from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import { toastSafeMessage } from "../lib/redact";
import type { AdvancedKind, AdvancedResult, AdvancedToolInfo } from "../types";

const FALLBACK_TOOLS: AdvancedToolInfo[] = [
  { kind: "scenario", name: "Scenario evaluation", description: "" },
  { kind: "grokRun", name: "Grok Run", description: "" },
  { kind: "grokBreaktest", name: "Grok Breaktest", description: "" },
];

export function AdvancedPage({ enabled, toast }: { enabled: boolean; toast: ToastApi }) {
  const { t } = useTranslation();
  const [tools, setTools] = useState<AdvancedToolInfo[]>(FALLBACK_TOOLS);
  const [kind, setKind] = useState<AdvancedKind>("scenario");
  const [args, setArgs] = useState("");
  const [result, setResult] = useState<AdvancedResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const label = (item: AdvancedKind) => {
    if (item === "scenario") return t("advanced.scenario");
    if (item === "grokRun") return t("advanced.grokRun");
    return t("advanced.grokBreaktest");
  };

  const load = useCallback(async () => {
    try {
      const listed = await api.listAdvancedTools();
      if (listed.tools?.length) setTools(listed.tools);
      setError(listed.enabled ? null : t("advanced.disabled"));
    } catch (err) {
      setError(toastSafeMessage(err));
    }
  }, [t]);

  useEffect(() => {
    void load();
  }, [load]);

  if (!enabled) {
    return <ErrorBanner message={t("advanced.disabled")} />;
  }

  const run = async () => {
    setBusy(true);
    try {
      const parsed: Record<string, string> = {};
      const trimmed = args.trim();
      if (trimmed) parsed.input = trimmed;
      const next = await api.runAdvanced(kind, parsed);
      setResult({
        ...next,
        output: toastSafeMessage(next.output),
        error: next.error ? toastSafeMessage(next.error) : null,
      });
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-3xl flex-col gap-2 overflow-auto">
      <Panel>
        <PanelHeader title={t("advanced.title")} />
        <div className="flex flex-col gap-2.5 px-3 py-2.5">
          <p className="text-[14px] text-muted-foreground">{t("advanced.hint")}</p>
          {error ? <ErrorBanner message={error} /> : null}

          <Segmented<AdvancedKind>
            value={kind}
            ariaLabel={t("advanced.title")}
            onChange={setKind}
            options={tools.map((item) => ({ value: item.kind, label: label(item.kind) }))}
          />

          <Field label={t("advanced.args")}>
            <Input
              value={args}
              disabled={busy}
              data-testid="advanced-args"
              onChange={(event) => setArgs(event.target.value)}
            />
          </Field>

          <Button
            variant="primary"
            className="self-start"
            disabled={busy}
            data-testid="advanced-run"
            onClick={() => void run()}
          >
            {busy ? t("common.busy") : t("advanced.run")}
          </Button>
        </div>
      </Panel>

      {result ? (
        <Panel>
          <PanelHeader title={t("advanced.output")} />
          <pre className="max-h-[50vh] overflow-auto whitespace-pre-wrap px-4 py-3 font-mono text-[13px] leading-snug">
            {result.output || result.error || t("common.empty")}
          </pre>
        </Panel>
      ) : null}
    </div>
  );
}
