import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useTheme } from "../hooks/useTheme";
import type { ToastApi } from "../hooks/useToasts";
import type { PromptDetail, ToolId } from "../types";
import { ErrorBanner } from "./ErrorBanner";
import { FullScreenPanel } from "./FullScreenPanel";
import { MarkdownEditor } from "./MarkdownEditor";
import { Button, Field, Input } from "./ui";
import * as api from "../api";

interface PromptDraft {
  title: string;
  content: string;
  tags: string;
}

export function PromptEditPage({
  tool,
  detail,
  promptId,
  creating,
  toast,
  onClose,
  onSaved,
  onDirtyChange,
}: {
  tool: ToolId;
  /** Existing callers may pass loaded detail; the app route normally passes promptId. */
  detail?: PromptDetail | null;
  promptId?: string;
  creating: boolean;
  toast: ToastApi;
  onClose: () => void;
  onSaved: (id: string) => void;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const { t } = useTranslation();
  const { resolved } = useTheme();
  const [loadedDetail, setLoadedDetail] = useState<PromptDetail | null>(detail ?? null);
  const [loading, setLoading] = useState(!creating && !detail && Boolean(promptId));
  const [loadError, setLoadError] = useState(false);
  const [draft, setDraft] = useState<PromptDraft>(() =>
    detail
      ? { title: detail.title, content: detail.content, tags: detail.tags.join(", ") }
      : { title: "", content: "", tags: "" },
  );
  const [saving, setSaving] = useState(false);
  const [loadEpoch, setLoadEpoch] = useState(0);

  useEffect(() => {
    if (detail) {
      setLoadedDetail(detail);
      setDraft({
        title: detail.title,
        content: detail.content,
        tags: detail.tags.join(", "),
      });
      setLoading(false);
      setLoadError(false);
    }
  }, [detail]);

  useEffect(() => {
    if (creating || detail || !promptId) return;
    let cancelled = false;
    setLoading(true);
    setLoadError(false);
    void api
      .getPrompt(promptId)
      .then((next) => {
        if (cancelled) return;
        setLoadedDetail(next);
        setDraft({ title: next.title, content: next.content, tags: next.tags.join(", ") });
      })
      .catch(() => {
        if (!cancelled) setLoadError(true);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [creating, detail, loadEpoch, promptId]);

  const isDirty = creating
    ? Boolean(draft.title || draft.content || draft.tags)
    : Boolean(
        loadedDetail &&
          (draft.title !== loadedDetail.title ||
            draft.content !== loadedDetail.content ||
            draft.tags !== loadedDetail.tags.join(", ")),
      );

  useEffect(() => {
    onDirtyChange?.(isDirty);
    return () => onDirtyChange?.(false);
  }, [isDirty, onDirtyChange]);

  const leavePage = () => {
    onDirtyChange?.(false);
    onClose();
  };

  const handleClose = () => {
    if (saving) return;
    if (isDirty && !window.confirm(t("unsaved.leave"))) return;
    leavePage();
  };

  const saveDraft = async () => {
    if (!draft.title.trim()) {
      toast.err(t("errors.validation"));
      return;
    }
    setSaving(true);
    const tagsArray = draft.tags
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean);
    try {
      if (creating) {
        const created = await api.createPrompt({
          tool,
          title: draft.title.trim(),
          content: draft.content,
          tags: tagsArray,
        });
        toast.ok(t("prompts.created"));
        onSaved(created.id);
      } else if (loadedDetail) {
        const updated = await api.updatePrompt({
          id: loadedDetail.id,
          title: draft.title.trim(),
          content: draft.content,
          tags: tagsArray,
        });
        toast.ok(t("prompts.updatedOk"));
        onSaved(updated.id);
      }
    } catch (err) {
      toast.err(err);
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <FullScreenPanel isOpen title={t("prompts.edit")} onClose={handleClose}>
        <div className="flex min-h-[240px] items-center justify-center text-sm text-muted-foreground" role="status">
          {t("common.loading")}
        </div>
      </FullScreenPanel>
    );
  }

  if (loadError || (!creating && !loadedDetail)) {
    return (
      <FullScreenPanel isOpen title={t("prompts.edit")} onClose={leavePage}>
        <div className="mx-auto w-full max-w-3xl">
          <ErrorBanner
            message={t("errors.loadFailed")}
            retryLabel={t("common.retry")}
            onRetry={() => setLoadEpoch((value) => value + 1)}
          />
        </div>
      </FullScreenPanel>
    );
  }

  return (
    <FullScreenPanel
      isOpen
      title={creating ? t("prompts.new") : t("prompts.edit")}
      onClose={handleClose}
      closeDisabled={saving}
      footer={
        <>
          <Button onClick={handleClose} disabled={saving}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            disabled={!draft.title.trim() || saving}
            data-testid="prompt-form-save"
            onClick={() => void saveDraft()}
          >
            {saving ? t("common.busy") : t("common.save")}
          </Button>
        </>
      }
    >
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-5">
        <Field label={t("prompts.title")}>
          <Input
            value={draft.title}
            disabled={saving}
            data-testid="prompt-form-title"
            onChange={(event) =>
              setDraft({ ...draft, title: event.target.value })
            }
          />
        </Field>
        <Field label={t("prompts.tags")}>
          <Input
            value={draft.tags}
            disabled={saving}
            onChange={(event) => setDraft({ ...draft, tags: event.target.value })}
          />
        </Field>
        <div>
          <div className="mb-2 flex items-center justify-between">
            <span className="text-sm font-medium text-foreground">
              {t("prompts.content")}
            </span>
          </div>
          <MarkdownEditor
            value={draft.content}
            onChange={(content) => setDraft({ ...draft, content })}
            ariaLabel={t("prompts.content")}
            darkMode={resolved === "dark"}
            readOnly={saving}
            minHeight="420px"
          />
        </div>
      </div>
    </FullScreenPanel>
  );
}
