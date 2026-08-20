import { useEffect, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
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
  onSave,
  onDataChanged,
  toast,
  initialTab = "general",
}: {
  settings: Settings;
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
  const [toolsLoading, setToolsLoading] = useState(true);
  const [toolsError, setToolsError] = useState(false);
  const [dirs, setDirs] = useState<DataDirs | null>(null);
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [backupsLoading, setBackupsLoading] = useState(true);
  const [backupsError, setBackupsError] = useState(false);
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

  const loadTools = async () => {
    setToolsLoading(true);
    setToolsError(false);
    try {
      const result = await api.listTools();
      setTools(result.tools ?? []);
    } catch {
      setTools([]);
      setToolsError(true);
    } finally {
      setToolsLoading(false);
    }
  };

  const loadBackups = async () => {
    setBackupsLoading(true);
    setBackupsError(false);
    try {
      const result = await api.listBackups();
      setBackups(result.backups ?? []);
    } catch {
      setBackups([]);
      setBackupsError(true);
    } finally {
      setBackupsLoading(false);
    }
  };

  useEffect(() => {
    void loadTools();
    void api.getDataDirs().then(setDirs).catch(() => setDirs(null));
    void loadBackups();
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

  const selectTab = (next: TabId, focus = false) => {
    setTab(next);
    if (focus) {
      requestAnimationFrame(() => {
        document.querySelector<HTMLElement>(`[data-testid="settings-nav-${next}"]`)?.focus();
      });
    }
  };

  const onTabKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>, item: TabId) => {
    const index = TABS.indexOf(item);
    let nextIndex: number | null = null;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % TABS.length;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + TABS.length) % TABS.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = TABS.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    selectTab(TABS[nextIndex]!, true);
  };

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-6xl flex-col gap-3">
      <div
        className="flex shrink-0 gap-1 overflow-x-auto rounded-xl border border-border bg-card p-1.5"
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
              tabIndex={active ? 0 : -1}
              aria-controls={`settings-panel-${item}`}
              data-testid={`settings-nav-${item}`}
              onClick={() => selectTab(item)}
              onKeyDown={(event) => onTabKeyDown(event, item)}
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

      <div
        id={`settings-panel-${tab}`}
        role="tabpanel"
        className="min-h-0 flex-1 overflow-auto rounded-xl border border-border bg-card shadow-[0_16px_50px_hsl(var(--foreground)/0.04)]"
        aria-busy={busy || undefined}
      >
        {tab === "general" ? (
          <div>
            <SettingRow
              label={t("settings.language")}
              control={
                <Select
                  aria-label={t("settings.language")}
                  value={settings.language}
                  disabled={busy}
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
                  disabled={busy}
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
              control={
                <Checkbox
                  aria-label={t("settings.closeToTray")}
                  checked={settings.closeToTray}
                  disabled={busy}
                  onChange={(event) => void patch({ closeToTray: event.target.checked })}
                />
              }
            />
            <SettingRow
              label={t("settings.autoLaunch")}
              control={
                <Checkbox
                  aria-label={t("settings.autoLaunch")}
                  checked={settings.autoLaunch}
                  disabled={busy}
                  onChange={(event) => void patch({ autoLaunch: event.target.checked })}
                />
              }
            />
            <SettingRow
              label={t("settings.silentStart")}
              control={
                <Checkbox
                  aria-label={t("settings.silentStart")}
                  checked={settings.silentStart}
                  disabled={busy}
                  onChange={(event) => void patch({ silentStart: event.target.checked })}
                />
              }
            />
            <SettingRow
              label={t("settings.defaultClaudeScope")}
              control={
                <Select
                  aria-label={t("settings.defaultClaudeScope")}
                  value={settings.defaultClaudeScope}
                  disabled={busy}
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
          toolsLoading ? (
            <div className="flex min-h-[220px] items-center justify-center text-sm text-muted-foreground" role="status">
              <span className="mr-2 h-3.5 w-3.5 animate-spin rounded-full border-2 border-primary/25 border-t-primary" aria-hidden="true" />
              {t("common.loading")}
            </div>
          ) : toolsError ? (
            <div className="p-4">
              <ErrorBanner
                message={t("settings.toolsLoadFailed")}
                onRetry={() => void loadTools()}
                retryLabel={t("common.retry")}
              />
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-3 p-4 sm:grid-cols-2">
              {TOOL_IDS.map((id) => {
                const info = tools.find((item) => item.id === id);
                return (
                  <div key={id} className="flex items-center gap-3 rounded-xl border border-border bg-background/45 px-4 py-3">
                    <ToolLogo tool={id} size={22} />
                    <div className="min-w-0 flex-1">
                      <p className="text-sm font-medium">{info?.name ?? id}</p>
                      <p className="mt-0.5 break-words text-xs text-muted-foreground">
                        {info?.available ? t("about.installed") : info?.unavailableReason || t("status.unavailable")}
                      </p>
                    </div>
                  </div>
                );
              })}
            </div>
          )
        ) : null}

        {tab === "data" ? (
          <div className="flex flex-col gap-5 p-4 sm:p-5">
            <div className="flex flex-wrap gap-2 rounded-xl border border-border bg-background/45 p-3">
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
            {backupsLoading ? (
              <div className="flex min-h-[120px] items-center justify-center rounded-xl border border-border text-sm text-muted-foreground" role="status" data-testid="data-backups-loading">
                <span className="mr-2 h-3.5 w-3.5 animate-spin rounded-full border-2 border-primary/25 border-t-primary" aria-hidden="true" />
                {t("common.loading")}
              </div>
            ) : backupsError ? (
              <ErrorBanner
                message={t("data.backupsLoadFailed")}
                onRetry={() => void loadBackups()}
                retryLabel={t("common.retry")}
              />
            ) : backups.length > 0 ? (
              <ul className="overflow-hidden rounded-xl border border-border text-sm">
                {backups.map((item) => (
                  <li key={item.id} className="flex items-center gap-2 border-b border-border px-3 py-2.5 last:border-b-0">
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
            ) : (
              <p className="rounded-xl border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground" data-testid="data-backups-empty">
                {t("data.noBackups")}
              </p>
            )}
            <Button
              variant="danger"
              className="self-start"
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
              control={
                <Select
                  aria-label={t("settings.updateChannel")}
                  value={settings.updateChannel}
                  disabled={busy}
                  onChange={(event) => void patch({ updateChannel: event.target.value as UpdateChannel })}
                >
                  <option value="stable">{t("settings.channelStable")}</option>
                  <option value="beta">{t("settings.channelBeta")}</option>
                </Select>
              }
            />
            <SettingRow
              label={t("settings.autoCheck")}
              control={
                <Checkbox
                  aria-label={t("settings.autoCheck")}
                  checked={settings.autoCheckUpdates}
                  disabled={busy}
                  onChange={(event) => void patch({ autoCheckUpdates: event.target.checked })}
                />
              }
            />
          </div>
        ) : null}

        {tab === "advanced" ? (
          <SettingRow
            label={t("settings.advancedTools")}
            control={
              <Checkbox
                aria-label={t("settings.advancedTools")}
                checked={settings.advancedToolsEnabled}
                disabled={busy}
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
        confirmLabel={busy ? t("common.busy") : t("data.clearConfirm")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        busy={busy}
        confirmDisabled={busy || clearPhrase !== clearPlan?.confirmPhrase || !clearConfirm}
        confirmTestId="clear-all-confirm"
        onClose={() => {
          if (!busy) setClearPlan(null);
        }}
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
        confirmLabel={busy ? t("common.busy") : pendingRestore?.source === "zip-import" ? t("data.importZip") : t("data.restore")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        busy={busy}
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
