import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ErrorBanner } from "../components/ErrorBanner";
import { Button, Card, Field, Input, Select } from "../components/ui";
import type { ToastApi } from "../hooks/useToasts";
import type { Language, ScopeId, Settings, SettingsPatch, UpdateChannel } from "../types";

export function SettingsPage({
  settings,
  loadError,
  onSave,
  toast,
}: {
  settings: Settings;
  loadError: string | null;
  onSave: (patch: SettingsPatch) => Promise<Settings>;
  toast: ToastApi;
}) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);

  const patch = async (next: SettingsPatch) => {
    setBusy(true);
    try {
      await onSave(next);
      toast.ok(t("settings.saved"));
    } catch (err) {
      toast.err(err);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-3">
      {loadError ? <ErrorBanner message={t("settings.loadFailed")} /> : null}

      <Card title={t("settings.title")}>
        <div className="grid gap-3">
          <Field label={t("settings.language")}>
            <Select
              value={settings.language}
              disabled={busy}
              onChange={(event) => void patch({ language: event.target.value as Language })}
            >
              <option value="zh-CN">{t("settings.languageZhCN")}</option>
              <option value="zh-TW">{t("settings.languageZhTW")}</option>
              <option value="en">{t("settings.languageEn")}</option>
            </Select>
          </Field>

          <Field label={t("settings.updateChannel")}>
            <Select
              value={settings.updateChannel}
              disabled={busy}
              onChange={(event) => void patch({ updateChannel: event.target.value as UpdateChannel })}
            >
              <option value="stable">{t("settings.channelStable")}</option>
              <option value="beta">{t("settings.channelBeta")}</option>
            </Select>
          </Field>

          <label className="flex items-start gap-2 text-[12px]">
            <input
              type="checkbox"
              className="mt-0.5"
              checked={settings.advancedToolsEnabled}
              disabled={busy}
              onChange={(event) => void patch({ advancedToolsEnabled: event.target.checked })}
            />
            <span>
              <span className="block font-medium text-ink-50">{t("settings.advancedTools")}</span>
              <span className="text-ink-200">{t("settings.advancedToolsHint")}</span>
            </span>
          </label>

          <Field label={t("settings.defaultClaudeScope")}>
            <Select
              value={settings.defaultClaudeScope}
              disabled={busy}
              onChange={(event) => void patch({ defaultClaudeScope: event.target.value as ScopeId })}
            >
              <option value="user">{t("scope.user")}</option>
              <option value="project">{t("scope.project")}</option>
              <option value="local">{t("scope.local")}</option>
            </Select>
          </Field>

          {settings.updaterEndpointOverride ? (
            <Field label={t("settings.endpointOverride")}>
              <Input value={settings.updaterEndpointOverride} readOnly />
            </Field>
          ) : null}
        </div>
      </Card>

      <Card title={t("settings.recentProjects")}>
        {settings.recentProjectDirs.length === 0 ? (
          <p className="text-[12px] text-ink-200">{t("settings.recentEmpty")}</p>
        ) : (
          <ul className="space-y-1">
            {settings.recentProjectDirs.map((dir) => (
              <li key={dir} className="flex items-center gap-2 text-[12px]">
                <span className="min-w-0 flex-1 truncate font-mono">{dir}</span>
                <Button
                  disabled={busy}
                  onClick={() =>
                    void patch({
                      recentProjectDirs: settings.recentProjectDirs.filter((item) => item !== dir),
                    })
                  }
                >
                  {t("settings.removeDir")}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
