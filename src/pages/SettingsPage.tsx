import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { AboutPage } from "./AboutPage";
import { ErrorBanner } from "../components/ErrorBanner";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { IconMonitor, IconMoon, IconSun, IconTrash } from "../components/icons";
import { ToolLogo } from "../components/ToolLogos";
import {
  Button,
  Checkbox,
  Input,
  Mono,
  Segmented,
  Select,
  SettingRow,
  cx,
} from "../components/ui";
import { useTheme, type ThemeMode } from "../hooks/useTheme";
import type { ToastApi } from "../hooks/useToasts";
import { shortPath } from "../lib/format";
import { openExternal, pickFiles, pickSavePath } from "../lib/runtime";
import type {
  BackupEntry,
  ClearPlan,
  DataDirs,
  Language,
  ScopeId,
  Settings,
  SettingsPatch,
  ToolInfo,
  UpdateChannel,
} from "../types";
import { TOOL_IDS } from "../types";

type TabId = "general" | "tools" | "data" | "updates" | "advanced" | "about";

const TABS: TabId[] = ["general", "tools", "data", "updates", "advanced", "about"];

export function SettingsPage({
  settings,
  loadError,
  onSave,
  toast,
  initialTab = "general",
}: {
  settings: Settings;
  loadError: string | null;
  onSave: (patch: SettingsPatch) => Promise<Settings>;
  toast: ToastApi;
  initialTab?: string;
}) {
  const { t } = useTranslation();
  const { theme, setTheme } = useTheme();
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<TabId>("general");
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [dirs, setDirs] = useState<DataDirs | null>(null);
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [clearPlan, setClearPlan] = useState<ClearPlan | null>(null);
  const [clearPhrase, setClearPhrase] = useState("");
  const [clearConfirm, setClearConfirm] = useState(false);

  useEffect(() => {
    if (TABS.includes(initialTab as TabId)) setTab(initialTab as TabId);
  }, [initialTab]);

  useEffect(() => {
    void api.listTools().then((result) => setTools(result.tools ?? [])).catch(() => setTools([]));
    void api.getDataDirs().then(setDirs).catch(() => setDirs(null));
    void api.listBackups().then((result) => setBackups(result.backups ?? [])).catch(() => setBackups([]));
  }, []);

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
    <div className="flex h-full min-h-0 flex-col gap-3">
      {loadError ? <ErrorBanner message={t("settings.loadFailed")} /> : null}
      <div
        className="flex shrink-0 gap-1 overflow-x-auto rounded-xl border border-border bg-muted p-1"
        role="tablist"
        aria-label={t("settings.title")}
      >
        {TABS.map((item) => {
          const active = item === tab;
          return (
            <button
              key={item}
              type="button"
              role="tab"
              aria-selected={active}
              data-testid={`settings-nav-${item}`}
              onClick={() => setTab(item)}
              className={cx(
                "h-9 shrink-0 rounded-lg px-3 text-sm font-medium transition-colors",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                active ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground",
              )}
            >
              {t(`settings.tab.${item}`)}
            </button>
          );
        })}
      </div>

      <div className="min-h-0 flex-1 overflow-auto rounded-lg border border-border bg-card">
        {tab === "general" ? (
          <div>
            <SettingRow
              label={t("settings.language")}
              control={
                <Select
                  value={settings.language}
                  onChange={(event) => void patch({ language: event.target.value as Language })}
                >
                  <option value="zh-CN">{t("settings.languageZhCN")}</option>
                  <option value="zh-TW">{t("settings.languageZhTW")}</option>
                  <option value="en">{t("settings.languageEn")}</option>
                </Select>
              }
            />
            <SettingRow
              label={t("settings.theme")}
              control={
                <Segmented
                  ariaLabel={t("settings.theme")}
                  value={theme}
                  onChange={(value) => {
                    setTheme(value);
                    void patch({ theme: value });
                  }}
                  options={[
                    { value: "light" as ThemeMode, label: <><IconSun size={14} />{t("settings.themeLight")}</> },
                    { value: "dark" as ThemeMode, label: <><IconMoon size={14} />{t("settings.themeDark")}</> },
                    { value: "system" as ThemeMode, label: <><IconMonitor size={14} />{t("settings.themeSystem")}</> },
                  ]}
                />
              }
            />
            <SettingRow
              label={t("settings.closeToTray")}
              description={t("settings.closeToTrayHint")}
              control={
                <Checkbox
                  label={t("settings.closeToTray")}
                  checked={settings.closeToTray}
                  onChange={(event) => void patch({ closeToTray: event.target.checked })}
                />
              }
            />
            <SettingRow
              label={t("settings.autoLaunch")}
              description={t("settings.autoLaunchHint")}
              control={
                <Checkbox
                  label={t("settings.autoLaunch")}
                  checked={settings.autoLaunch}
                  onChange={(event) => void patch({ autoLaunch: event.target.checked })}
                />
              }
            />
            <SettingRow
              label={t("settings.silentStart")}
              description={t("settings.silentStartHint")}
              control={
                <Checkbox
                  label={t("settings.silentStart")}
                  checked={settings.silentStart}
                  onChange={(event) => void patch({ silentStart: event.target.checked })}
                />
              }
            />
            <SettingRow
              label={t("settings.defaultClaudeScope")}
              control={
                <Select
                  value={settings.defaultClaudeScope}
                  onChange={(event) => void patch({ defaultClaudeScope: event.target.value as ScopeId })}
                >
                  <option value="user">{t("scope.user")}</option>
                  <option value="project">{t("scope.project")}</option>
                  <option value="local">{t("scope.local")}</option>
                </Select>
              }
            />
          </div>
        ) : null}

        {tab === "tools" ? (
          <div className="divide-y divide-border">
            {TOOL_IDS.map((id) => {
              const info = tools.find((item) => item.id === id);
              return (
                <div key={id} className="flex items-start gap-3 px-4 py-3">
                  <ToolLogo tool={id} size={20} />
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium">{info?.name ?? id}</p>
                    <p className="text-sm text-muted-foreground">
                      {info?.available ? t("about.installed") : info?.unavailableReason || t("status.unavailable")}
                    </p>
                    {info?.cliPath ? <Mono>{shortPath(info.cliPath)}</Mono> : null}
                    <p className="text-xs text-muted-foreground">v{info?.adapterVersion}</p>
                  </div>
                </div>
              );
            })}
          </div>
        ) : null}

        {tab === "data" ? (
          <div className="flex flex-col gap-3 p-4">
            <div className="flex flex-wrap gap-2">
              <Button
                onClick={() => {
                  void (async () => {
                    const files = await pickFiles([{ name: "Markdown", extensions: ["md"] }]);
                    if (!files.length) return;
                    const result = await api.importMarkdownFiles("claude", files);
                    toast.ok(`${result.imported}`);
                  })();
                }}
              >
                {t("data.importMarkdown")}
              </Button>
              <Button
                onClick={() => {
                  void (async () => {
                    const files = await pickFiles([{ name: "ZIP", extensions: ["zip"] }]);
                    if (!files[0]) return;
                    const result = await api.importZipArchive(files[0]);
                    toast.ok(`${result.imported}`);
                  })();
                }}
              >
                {t("data.importZip")}
              </Button>
              <Button
                onClick={() => {
                  void (async () => {
                    const path = await pickSavePath("keysmith-switch-export.zip");
                    if (!path) return;
                    await api.exportZipArchive(path);
                    toast.ok(t("data.exported"));
                  })();
                }}
              >
                {t("data.exportZip")}
              </Button>
              <Button
                onClick={() => {
                  void (async () => {
                    const entry = await api.createBackup();
                    setBackups((current) => [entry, ...current]);
                    toast.ok(t("data.backupCreated"));
                  })();
                }}
              >
                {t("data.backupNow")}
              </Button>
              {dirs ? (
                <>
                  <Button onClick={() => void openExternal(`file://${dirs.home}`)}>{t("data.openHome")}</Button>
                  <Button onClick={() => void openExternal(`file://${dirs.logs}`)}>{t("data.openLogs")}</Button>
                </>
              ) : null}
            </div>
            <ul className="text-sm">
              {backups.map((item) => (
                <li key={item.id} className="flex items-center gap-2 border-b border-border py-2">
                  <span className="min-w-0 flex-1 truncate">{item.id}</span>
                  <Button
                    size="sm"
                    onClick={() => {
                      void api.restoreBackup(item.path).then((result) => toast.ok(`${result.imported}`));
                    }}
                  >
                    {t("data.restore")}
                  </Button>
                </li>
              ))}
            </ul>
            <Button
              variant="danger"
              disabled={busy}
              onClick={() => {
                void api.planClearAllData().then((plan) => {
                  setClearPlan(plan);
                  setClearPhrase("");
                  setClearConfirm(false);
                });
              }}
            >
              <IconTrash />
              {t("data.clearAll")}
            </Button>
          </div>
        ) : null}

        {tab === "updates" ? (
          <div>
            <SettingRow
              label={t("settings.updateChannel")}
              description={t("settings.updateChannelHint")}
              control={
                <Select
                  value={settings.updateChannel}
                  onChange={(event) => void patch({ updateChannel: event.target.value as UpdateChannel })}
                >
                  <option value="stable">{t("settings.channelStable")}</option>
                  <option value="beta">{t("settings.channelBeta")}</option>
                </Select>
              }
            />
            <SettingRow
              label={t("settings.autoCheck")}
              description={t("about.autoCheck")}
              control={
                <Checkbox
                  label={t("settings.autoCheck")}
                  checked={settings.autoCheckUpdates}
                  onChange={(event) => void patch({ autoCheckUpdates: event.target.checked })}
                />
              }
            />
          </div>
        ) : null}

        {tab === "advanced" ? (
          <SettingRow
            label={t("settings.advancedTools")}
            description={t("settings.advancedToolsHint")}
            control={
              <Checkbox
                label={t("settings.advancedTools")}
                checked={settings.advancedToolsEnabled}
                onChange={(event) => void patch({ advancedToolsEnabled: event.target.checked })}
              />
            }
          />
        ) : null}

        {tab === "about" ? <AboutPage channel={settings.updateChannel} toast={toast} autoCheckDelayMs={null} /> : null}
      </div>

      <ConfirmDialog
        open={Boolean(clearPlan)}
        title={t("data.clearAll")}
        description={t("data.clearAllHint")}
        danger
        confirmLabel={t("data.clearConfirm")}
        cancelLabel={t("common.cancel")}
        confirmDisabled={clearPhrase !== clearPlan?.confirmPhrase || !clearConfirm}
        confirmTestId="clear-all-confirm"
        onClose={() => setClearPlan(null)}
        onConfirm={() => {
          if (!clearPlan) return;
          void (async () => {
            await api.clearAllData(clearPhrase);
            toast.ok(t("data.cleared"));
            setClearPlan(null);
          })();
        }}
      >
        {clearPlan ? (
          <div className="space-y-2 text-sm">
            {clearPlan.categories.map((item) => (
              <p key={item.name}>
                <span className="font-medium">{item.name}</span> · <Mono>{item.path}</Mono>
              </p>
            ))}
            <Input
              value={clearPhrase}
              placeholder={clearPlan.confirmPhrase}
              onChange={(event) => setClearPhrase(event.target.value)}
            />
            <Checkbox
              label={t("data.clearSecond")}
              checked={clearConfirm}
              onChange={(event) => setClearConfirm(event.target.checked)}
            />
          </div>
        ) : null}
      </ConfirmDialog>
    </div>
  );
}
