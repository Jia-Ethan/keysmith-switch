import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ErrorBanner } from "../components/ErrorBanner";
import { Button, Card, Field, Input } from "../components/ui";
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
    <div className="mx-auto flex max-w-3xl flex-col gap-3">
      <Card title={t("advanced.title")}>
        <p className="mb-3 text-[12px] text-ink-200">{t("advanced.hint")}</p>
        {error ? <ErrorBanner message={error} /> : null}
        <div className="mb-3 flex flex-wrap gap-1">
          {tools.map((item) => (
            <Button
              key={item.kind}
              variant={kind === item.kind ? "primary" : "outline"}
              onClick={() => setKind(item.kind)}
            >
              {label(item.kind)}
            </Button>
          ))}
        </div>
        <Field label={t("advanced.args")}>
          <Input value={args} onChange={(event) => setArgs(event.target.value)} />
        </Field>
        <Button className="mt-2" variant="primary" disabled={busy} onClick={() => void run()}>
          {t("advanced.run")}
        </Button>
      </Card>
      {result ? (
        <Card title={t("advanced.output")}>
          <pre className="overflow-auto whitespace-pre-wrap font-mono text-[11px]">
            {result.output || result.error || t("common.empty")}
          </pre>
        </Card>
      ) : null}
    </div>
  );
}
