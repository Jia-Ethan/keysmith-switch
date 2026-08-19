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
  ToolId,
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
  onDataChanged,
  toast,
  initialTab = "general",
}: {
  settings: Settings;
  loadError: string | null;
  onSave: (patch: SettingsPatch) => Promise<Settings>;
  onDataChanged?: () => Promise<void> | void;
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
  const [importTool, setImportTool] = useState<ToolId>("claude");
  const [pendingRestore, setPendingRestore] = useState<{
    path: string;
    source: "backup" | "zip-restore" | "zip-import";
    label: string;
  } | null>(null);

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

  const runDataAction = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
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
              <Select
                aria-label={t("data.importTarget")}
                value={importTool}
                disabled={busy}
                onChange={(event) => setImportTool(event.target.value as ToolId)}
              >
                {TOOL_IDS.map((tool) => (
                  <option key={tool} value={tool}>
                    {t(`nav.${tool}`)}
                  </option>
                ))}
              </Select>
              <Button
                disabled={busy}
                onClick={() => {
                  void (async () => {
                    try {
                      const files = await pickFiles([{ name: "Markdown", extensions: ["md"] }]);
                      if (!files.length) return;
                      await runDataAction(async () => {
                        const result = await api.importMarkdownFiles(importTool, files);
                        if (result.errors.length) toast.err(result.errors.join("; "));
                        if (result.imported > 0) {
                          toast.ok(t("data.importedCount", { count: result.imported }));
                          await onDataChanged?.();
                        } else if (result.errors.length === 0) {
                          toast.info(t("data.nothingImported"));
                        }
                      });
                    } catch (err) {
                      toast.err(err);
                    }
                  })();
                }}
              >
                {t("data.importMarkdown")}
              </Button>
              <Button
                disabled={busy}
                onClick={() => {
                  void (async () => {
                    try {
                      const files = await pickFiles([{ name: "ZIP", extensions: ["zip"] }]);
                      if (!files[0]) return;
                      await runDataAction(async () => {
                        const inspection = await api.inspectZipArchive(files[0]);
                        setPendingRestore({
                          path: files[0],
                          source: inspection.mode === "restore" ? "zip-restore" : "zip-import",
                          label: files[0].split(/[\\/]/).pop() || files[0],
                        });
                      });
                    } catch (err) {
                      toast.err(err);
                    }
                  })();
                }}
              >
                {t("data.importZip")}
              </Button>
              <Button
                disabled={busy}
                onClick={() => {
                  void (async () => {
                    try {
                      const path = await pickSavePath("keysmith-switch-export.zip");
                      if (!path) return;
                      await runDataAction(async () => {
                        await api.exportZipArchive(path);
                        toast.ok(t("data.exported"));
                      });
                    } catch (err) {
                      toast.err(err);
                    }
                  })();
                }}
              >
                {t("data.exportZip")}
              </Button>
              <Button
                disabled={busy}
                onClick={() => {
                  void runDataAction(async () => {
                    const entry = await api.createBackup();
                    setBackups((current) => [entry, ...current]);
                    toast.ok(t("data.backupCreated"));
                  });
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
                    disabled={busy}
                    onClick={() => {
                      setPendingRestore({ path: item.path, source: "backup", label: item.id });
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
                void runDataAction(async () => {
                  const plan = await api.planClearAllData();
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

        {tab === "about" ? <AboutPage channel={settings.updateChannel} toast={toast} /> : null}
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
          void runDataAction(async () => {
            await api.clearAllData(clearPhrase);
            toast.ok(t("data.cleared"));
            setClearPlan(null);
            try {
              await onDataChanged?.();
            } catch (err) {
              toast.err(err);
            }
          });
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

      <ConfirmDialog
        open={Boolean(pendingRestore)}
        title={pendingRestore?.source === "zip-import" ? t("data.importLegacyTitle") : t("data.restoreTitle")}
        description={pendingRestore?.source === "zip-import" ? t("data.importLegacyHint") : t("data.restoreHint")}
        danger={pendingRestore?.source !== "zip-import"}
        confirmLabel={pendingRestore?.source === "zip-import" ? t("data.importZip") : t("data.restore")}
        cancelLabel={t("common.cancel")}
        confirmDisabled={busy}
        confirmTestId="restore-backup-confirm"
        onClose={() => {
          if (!busy) setPendingRestore(null);
        }}
        onConfirm={() => {
          if (!pendingRestore) return;
          const request = pendingRestore;
          void runDataAction(async () => {
            const result = request.source === "backup"
              ? await api.restoreBackup(request.path)
              : await api.importZipArchive(request.path);
            if (result.errors.length) toast.err(result.errors.join("; "));
            toast.ok(t("data.archiveAppliedCount", { count: result.imported }));
            setPendingRestore(null);
            try {
              await onDataChanged?.();
            } catch (err) {
              toast.err(err);
            }
          });
        }}
      >
        {pendingRestore ? (
          <div className="space-y-2 text-sm">
            <p className="font-medium text-foreground">{pendingRestore.label}</p>
            <p className="text-muted-foreground">
              {pendingRestore.source === "zip-import" ? t("data.importLegacyScope") : t("data.restoreScope")}
            </p>
          </div>
        ) : null}
      </ConfirmDialog>
    </div>
  );
}
