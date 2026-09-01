import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useTheme } from "../hooks/useTheme";
import type { PromptDraft } from "./PromptEditor";
import { FullScreenPanel } from "./FullScreenPanel";
import { MarkdownEditor } from "./MarkdownEditor";
import { Button, Field, Input } from "./ui";

export function PromptFormPanel({
  title,
  draft,
  saving,
  onChange,
  onSave,
  onClose,
  dirtyGuard,
}: {
  title: string;
  draft: PromptDraft;
  saving: boolean;
  onChange: (draft: PromptDraft) => void;
  onSave: () => void;
  onClose: () => void;
  dirtyGuard: () => boolean;
}) {
  const { t } = useTranslation();
  const { resolved } = useTheme();
  const [local, setLocal] = useState(draft);

  useEffect(() => {
    setLocal(draft);
  }, [draft]);

  const update = (next: PromptDraft) => {
    setLocal(next);
    onChange(next);
  };

  const handleClose = () => {
    if (saving) return;
    if (!dirtyGuard()) return;
    onClose();
  };

  return (
    <FullScreenPanel
      isOpen
      title={title}
      onClose={handleClose}
      footer={
        <>
          <Button onClick={handleClose} disabled={saving}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="primary"
            disabled={!local.title.trim() || saving}
            data-testid="prompt-form-save"
            onClick={onSave}
          >
            {saving ? t("common.busy") : t("common.save")}
          </Button>
        </>
      }
    >
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-5">
        <Field label={t("prompts.title")}>
          <Input
            value={local.title}
            data-testid="prompt-form-title"
            onChange={(event) => update({ ...local, title: event.target.value })}
          />
        </Field>
        <Field label={t("prompts.tags")}>
          <Input
            value={local.tags}
            onChange={(event) => update({ ...local, tags: event.target.value })}
          />
        </Field>
        <div>
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[14px] font-medium text-muted-foreground">{t("prompts.content")}</span>
          </div>
          <MarkdownEditor
            value={local.content}
            onChange={(content) => update({ ...local, content })}
            darkMode={resolved === "dark"}
            readOnly={saving}
            minHeight="420px"
          />
        </div>
      </div>
    </FullScreenPanel>
  );
}
