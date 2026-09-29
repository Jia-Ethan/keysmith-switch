import { useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { openExternal } from "../lib/runtime";
import type { FeedbackInput, FeedbackResult } from "../types";
import { IconExternal, IconInfo } from "./icons";
import { ConfirmDialog } from "./ConfirmDialog";
import { Button, Field, Input, SectionLabel, Textarea } from "./ui";

type Kind = FeedbackInput["kind"];
const EMPTY = { description: "", solution: "", contact: "" };

/**
 * Feedback goes to the public repository's issue forms. The backend creates
 * the issue directly when a signed-in GitHub CLI is available; otherwise it
 * returns the prefilled form, which opens in the browser straight away so the
 * person reviews and submits it under their own account.
 */
export function Feedback() {
  const { t } = useTranslation();
  const [kind, setKind] = useState<Kind | null>(null);
  const [drafts, setDrafts] = useState<Record<Kind, typeof EMPTY>>({ bug: { ...EMPTY }, feature: { ...EMPTY } });
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<FeedbackResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const draft = kind ? drafts[kind] : EMPTY;
  const patch = (next: Partial<typeof EMPTY>) => {
    if (!kind) return;
    setDrafts((current) => ({ ...current, [kind]: { ...current[kind], ...next } }));
    setResult(null);
    setError(null);
  };
  const open = (next: Kind) => {
    setKind(next);
    setResult(null);
    setError(null);
  };
  const ready = Boolean(kind && draft.description.trim() && (kind !== "feature" || draft.solution.trim()));
  const submit = async () => {
    if (!kind || !ready) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const response = await api.submitFeedback({ kind, ...draft, screenshots: [] });
      setResult(response);
      if (response.issueUrl) {
        setDrafts((current) => ({ ...current, [kind]: { ...EMPTY } }));
      } else if (response.fallbackUrl) {
        await openExternal(response.fallbackUrl);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t("feedback.createFailed"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="border-b border-border p-4 sm:p-5" data-testid="feedback-section">
      <SectionLabel>{t("feedback.title")}</SectionLabel>
      <div className="mt-2 flex flex-wrap gap-2">
        <Button size="sm" onClick={() => open("bug")}>{t("feedback.bug")}</Button>
        <Button size="sm" onClick={() => open("feature")}>{t("feedback.feature")}</Button>
      </div>
      <ConfirmDialog
        open={kind !== null}
        title={t(kind === "feature" ? "feedback.feature" : "feedback.bug")}
        confirmLabel={busy ? t("common.busy") : t("feedback.submit")}
        cancelLabel={t("common.close")}
        closeLabel={t("common.close")}
        busy={busy}
        wide
        confirmDisabled={busy || !ready || Boolean(result?.issueUrl)}
        onConfirm={() => void submit()}
        onClose={() => setKind(null)}
      >
        <div className="space-y-4">
          <div className="flex items-start gap-2 rounded-xl border border-border bg-muted/50 px-3 py-2 text-[12.5px] leading-relaxed text-muted-foreground">
            <IconInfo size={14} className="mt-[3px] shrink-0" />
            <div className="min-w-0 space-y-0.5">
              <p data-testid="feedback-how">{t("feedback.how")}</p>
              <p>{t("feedback.publicWarning")}</p>
            </div>
          </div>
          <Field label={t(kind === "feature" ? "feedback.requestDescription" : "feedback.problemDescription")}>
            <Textarea value={draft.description} required maxLength={10000} rows={5} onChange={(event) => patch({ description: event.target.value })} />
          </Field>
          {kind === "feature" ? (
            <Field label={t("feedback.solution")}>
              <Textarea value={draft.solution} required maxLength={10000} rows={4} onChange={(event) => patch({ solution: event.target.value })} />
            </Field>
          ) : (
            <p className="text-[12.5px] text-muted-foreground" data-testid="feedback-screenshot-tip">{t("feedback.screenshotTip")}</p>
          )}
          <Field label={t("feedback.contact")}>
            <Input value={draft.contact} maxLength={500} onChange={(event) => patch({ contact: event.target.value })} />
          </Field>
          {error ? <p role="alert" className="text-sm text-destructive">{error}</p> : null}
          {result?.issueUrl ? (
            <div role="status" className="space-y-1 text-sm">
              <p>{t("feedback.created")}</p>
              <Button size="sm" onClick={() => void openExternal(result.issueUrl!)}><IconExternal />{t("feedback.openIssue")}</Button>
            </div>
          ) : null}
          {result?.fallbackUrl ? (
            <div role="status" className="space-y-2 text-sm">
              <p>{t(result.reason === "createFailed" ? "feedback.createFailed" : "feedback.browser")}</p>
              {result.truncated ? <p className="text-warning">{t("feedback.truncated")}</p> : null}
              <Button size="sm" onClick={() => void openExternal(result.fallbackUrl!)}><IconExternal />{t("feedback.reopen")}</Button>
            </div>
          ) : null}
        </div>
      </ConfirmDialog>
    </section>
  );
}
