import { useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { openExternal, pickFiles } from "../lib/runtime";
import type { FeedbackInput, FeedbackResult } from "../types";
import { IconExternal } from "./icons";
import { ConfirmDialog } from "./ConfirmDialog";
import { Button, Field, Input, SectionLabel, Textarea } from "./ui";

type Kind = FeedbackInput["kind"];
const EMPTY = { description: "", solution: "", contact: "", screenshots: [] as string[] };

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
  const addScreenshots = async () => {
    const selected = await pickFiles([{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }]);
    if (!kind || !selected.length) return;
    const files = [...new Set([...drafts[kind].screenshots, ...selected])];
    if (files.length > 3) {
      setError(t("feedback.screenshotLimit"));
      return;
    }
    patch({ screenshots: files });
  };
  const submit = async () => {
    if (!kind || !draft.description.trim() || (kind === "feature" && !draft.solution.trim())) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const response = await api.submitFeedback({ kind, ...draft });
      setResult(response);
      if (response.issueUrl) {
        setDrafts((current) => ({ ...current, [kind]: { ...EMPTY, screenshots: [] } }));
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
        confirmDisabled={busy || !draft.description.trim() || (kind === "feature" && !draft.solution.trim()) || Boolean(result?.issueUrl)}
        onConfirm={() => void submit()}
        onClose={() => setKind(null)}
      >
        <div className="space-y-4">
          <p className="text-sm text-muted-foreground">{t("feedback.publicWarning")}</p>
          <Field label={t(kind === "feature" ? "feedback.requestDescription" : "feedback.problemDescription")}>
            <Textarea value={draft.description} required maxLength={10000} rows={5} onChange={(event) => patch({ description: event.target.value })} />
          </Field>
          {kind === "feature" ? (
            <Field label={t("feedback.solution")}>
              <Textarea value={draft.solution} required maxLength={10000} rows={4} onChange={(event) => patch({ solution: event.target.value })} />
            </Field>
          ) : (
            <div>
              <span className="text-sm font-medium text-muted-foreground">{t("feedback.screenshots")}</span>
              <div className="mt-1 flex flex-wrap items-center gap-2">
                <Button size="sm" disabled={draft.screenshots.length >= 3} onClick={() => void addScreenshots()}>{t("feedback.addScreenshot")}</Button>
                <span className="text-xs text-muted-foreground">{t("feedback.screenshotHint")}</span>
              </div>
              {draft.screenshots.length ? (
                <ul className="mt-2 space-y-1 text-sm">
                  {draft.screenshots.map((path) => (
                    <li key={path} className="flex min-w-0 items-center gap-2">
                      <span className="min-w-0 flex-1 truncate" title={path}>{path.split(/[\\/]/).pop()}</span>
                      <Button size="sm" onClick={() => patch({ screenshots: draft.screenshots.filter((item) => item !== path) })}>{t("feedback.remove")}</Button>
                    </li>
                  ))}
                </ul>
              ) : null}
            </div>
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
              <p>{t(`feedback.${result.reason ?? "createFailed"}`)}</p>
              {result.reason === "screenshotsManual" ? <p>{t("feedback.manualScreenshot")}</p> : null}
              <Button size="sm" onClick={() => void openExternal(result.fallbackUrl!)}><IconExternal />{t("feedback.openDraft")}</Button>
            </div>
          ) : null}
        </div>
      </ConfirmDialog>
    </section>
  );
}
