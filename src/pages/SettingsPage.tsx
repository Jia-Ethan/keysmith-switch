import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent, ReactNode } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ErrorBanner } from "../components/ErrorBanner";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { useUpdateOptional } from "../components/UpdateProvider";
import { IconAlert, IconDownload, IconExternal, IconMonitor, IconMoon, IconRefresh, IconSun, IconTrash } from "../components/icons";
import { ToolLogo } from "../components/ToolLogos";
import {
  Button,
  Checkbox,
  Disclosure,
  Input,
  Mono,
  Segmented,
  Select,
  SettingRow,
  SectionLabel,
  cx,
} from "../components/ui";
import { useTheme, type ThemeMode } from "../hooks/useTheme";
import type { ToastApi } from "../hooks/useToasts";
import { formatArgv, formatBytes } from "../lib/format";
import { toastSafeMessage } from "../lib/redact";
import { isTauriRuntime, openExternal, pickFiles, pickSavePath } from "../lib/runtime";
import type {
  AboutInfo,
  BackupEntry,
  ClearPlan,
  DataDirs,
  Language,
  OfficialAction,
  OfficialPlan,
  OfficialProduct,
  OfficialProductId,
  ScopeId,
  Settings,
  SettingsPatch,
  ToolId,
} from "../types";
import { TOOL_IDS } from "../types";

type TabId = "general" | "tools" | "data" | "about";

const TABS: TabId[] = ["general", "tools", "data", "about"];

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
  const PROJECT_REPO = "https://github.com/Jia-Ethan/keysmith-switch";
  const LICENSE_URL = "https://github.com/Jia-Ethan/keysmith-switch/blob/main/LICENSE";

  const { t } = useTranslation();
  const { theme, setTheme } = useTheme();
  const updater = useUpdateOptional();
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<TabId>("general");
  const [about, setAbout] = useState<AboutInfo | null>(null);
  const [toolsLoading, setToolsLoading] = useState(true);
  const [toolsError, setToolsError] = useState<string | null>(null);
  const [officialPlan, setOfficialPlan] = useState<OfficialPlan | null>(null);
  const [officialConfirmed, setOfficialConfirmed] = useState(false);
  const [officialBusy, setOfficialBusy] = useState(false);
  const [cancelPending, setCancelPending] = useState(false);
  const [officialElapsed, setOfficialElapsed] = useState(0);
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const updateInstallPending = useRef(false);
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

  useEffect(() => {
    setUpdateDialogOpen(false);
  }, [updater?.update?.latestVersion]);

  const loadTools = useCallback(async () => {
    setToolsLoading(true);
    setToolsError(null);
    try {
      setAbout(await api.getAbout());
    } catch (err) {
      setAbout(null);
      setToolsError(toastSafeMessage(err) || t("settings.toolsLoadFailed"));
    } finally {
      setToolsLoading(false);
    }
  }, [t]);

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
  }, [loadTools]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/event").then(({ listen }) => {
      if (cancelled) return;
      void listen<{ elapsedSeconds?: number }>("official-action-progress", (event) => {
        setOfficialElapsed(Math.max(0, event.payload.elapsedSeconds ?? 0));
      }).then((next) => {
        if (cancelled) next();
        else unlisten = next;
      });
    }).catch(() => undefined);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const previewOfficial = async (product: OfficialProductId, action: OfficialAction) => {
    setOfficialBusy(true);
    setOfficialConfirmed(false);
    try {
      setOfficialPlan(await api.planOfficialAction(product, action));
    } catch (err) {
      toast.err(err);
      setOfficialPlan(null);
    } finally {
      setOfficialBusy(false);
    }
  };

  const runOfficial = async () => {
    if (!officialPlan || !officialConfirmed) return;
    setOfficialBusy(true);
    setOfficialElapsed(0);
    try {
      const result = await api.confirmOfficialAction(officialPlan.planId);
      if (!result.ok) {
        toast.err(result.error || t("about.officialBlocked"));
        return;
      }
      toast.ok(t("common.success"));
      setOfficialPlan(null);
      setOfficialConfirmed(false);
      await loadTools();
    } catch (err) {
      toast.err(err);
    } finally {
      setOfficialBusy(false);
      setOfficialElapsed(0);
    }
  };

  const cancelOfficial = async () => {
    if (cancelPending) return;
    setCancelPending(true);
    try {
      const result = await api.cancelOfficialAction();
      if (!result.cancelled) toast.info(t("about.cancelOfficialUnavailable"));
    } catch (err) {
      toast.err(err);
    } finally {
      setCancelPending(false);
    }
  };

  const installUpdate = async () => {
    if (
      !updater?.update?.available
      || updater.update.installMode === "manual"
      || updater.installing
      || updateInstallPending.current
    ) return;
    updateInstallPending.current = true;
    try {
      const result = await updater.install();
      if (result?.installMode === "manual") {
        setUpdateDialogOpen(false);
        return;
      }
      if (!result?.ok) {
        toast.err(result?.error || t("about.updateFailed"));
        return;
      }
      setUpdateDialogOpen(false);
      toast.ok(t("common.success"));
    } finally {
      updateInstallPending.current = false;
    }
  };

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
                message={toolsError}
                onRetry={() => void loadTools()}
                retryLabel={t("common.retry")}
              />
            </div>
          ) : about ? (
            <div className="flex flex-col gap-5 p-4 sm:p-5">
              <section aria-label={t("about.adapters")}>
                <SectionLabel>{t("about.adapters")}</SectionLabel>
                <div className="mt-2 overflow-hidden rounded-xl border border-border">
                  {about.adapters.length > 0 ? (
                    about.adapters.map((item) => (
                      <div
                        key={item.tool}
                        className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-border bg-background/35 px-3 py-2.5 last:border-b-0"
                      >
                        <ToolLogo tool={item.tool} size={20} />
                        <span className="min-w-[72px] text-sm font-medium capitalize text-foreground">
                          {item.tool}
                        </span>
                        <Mono className="text-foreground">{item.version}</Mono>
                        {item.path ? (
                          <Mono className="ml-auto max-w-full truncate">
                            <span title={item.path}>{item.path}</span>
                          </Mono>
                        ) : null}
                      </div>
                    ))
                  ) : (
                    <p className="px-3 py-4 text-center text-sm text-muted-foreground" data-testid="settings-adapters-empty">
                      {t("common.none")}
                    </p>
                  )}
                </div>
              </section>

              <section aria-label={t("about.official")}>
                <SectionLabel>{t("about.official")}</SectionLabel>
                <div className="mt-2 flex flex-col gap-2">
                  {about.official.length > 0 ? (
                    about.official.map((product) => (
                      <OfficialToolRow
                        key={product.product}
                        product={product}
                        busy={officialBusy}
                        onPlan={previewOfficial}
                      />
                    ))
                  ) : (
                    <p className="rounded-xl border border-border px-3 py-4 text-center text-sm text-muted-foreground" data-testid="settings-official-empty">
                      {t("common.none")}
                    </p>
                  )}
                </div>

                {officialPlan ? (
                  <div className="mt-3 rounded-xl border border-border bg-background/45 p-3 text-[13px]" data-testid="official-plan">
                    <p className="font-medium capitalize text-foreground">
                      {officialPlan.product} / {officialPlan.action}
                    </p>
                    <dl className="mt-2 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1.5">
                      <DetailRow label={t("about.command")}><Mono>{formatArgv(officialPlan.argv)}</Mono></DetailRow>
                      <DetailRow label={t("about.dest")}><Mono>{officialPlan.dest || "—"}</Mono></DetailRow>
                      <DetailRow label={t("about.source")}>{officialPlan.source || "—"}</DetailRow>
                    </dl>
                    {officialPlan.blockers.length > 0 ? (
                      <div className="mt-2 flex items-start gap-2 rounded border border-destructive/40 bg-destructive/10 px-2 py-1.5 text-destructive">
                        <IconAlert size={14} className="mt-px shrink-0" />
                        <ul className="min-w-0 list-inside list-disc">
                          {officialPlan.blockers.map((item, index) => <li key={index}>{item}</li>)}
                        </ul>
                      </div>
                    ) : (
                      <div className="mt-3 flex flex-wrap items-center gap-2">
                        <Checkbox
                          checked={officialConfirmed}
                          data-testid="confirm-official"
                          label={t("about.confirmOfficial")}
                          disabled={officialBusy}
                          onChange={(event) => setOfficialConfirmed(event.target.checked)}
                        />
                        {officialBusy ? (
                          <Button
                            size="sm"
                            className="ml-auto"
                            data-testid="cancel-official"
                            disabled={cancelPending}
                            onClick={() => void cancelOfficial()}
                          >
                            {cancelPending ? t("common.busy") : t("about.cancelOfficial")}
                          </Button>
                        ) : (
                          <Button
                            size="sm"
                            variant="primary"
                            className="ml-auto"
                            data-testid="run-official"
                            disabled={!officialConfirmed}
                            onClick={() => void runOfficial()}
                          >
                            {t("about.runOfficial")}
                          </Button>
                        )}
                        {officialBusy ? (
                          <span className="text-muted-foreground">
                            {t("about.officialRunning", { seconds: officialElapsed })}
                          </span>
                        ) : null}
                      </div>
                    )}
                  </div>
                ) : null}
              </section>

              <div className="border-t border-border pt-4">
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
              </div>
            </div>
          ) : null
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

        {tab === "about" ? (
          <div>
            <section className="border-b border-border p-4 sm:p-5" data-testid="update-section">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <SectionLabel>{t("about.appUpdate")}</SectionLabel>
                  <p className="mt-1 text-sm text-foreground">
                    {updater?.update?.available
                      ? `${updater.update.currentVersion} → ${updater.update.latestVersion ?? "—"}`
                      : updater?.update?.currentVersion ?? about?.app.version ?? "—"}
                  </p>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  data-testid="check-update"
                  disabled={!updater || updater.checking || updater.installing}
                  onClick={() => void updater?.check()}
                >
                  <IconRefresh />
                  {updater?.checking ? t("about.checking") : t("about.checkUpdate")}
                </Button>
              </div>

              {updater?.error && updater.update?.installMode !== "manual" ? (
                <div className="mt-3">
                  <ErrorBanner
                    message={updater.error}
                    onRetry={() => void updater.check()}
                    retryLabel={t("common.retry")}
                  />
                </div>
              ) : null}

              {(!updater?.error || updater.update?.installMode === "manual") && updater?.update && !updater.update.available ? (
                <p className="mt-3 text-sm text-primary">
                  {updater.update.currentVersion} · {t("about.upToDate")}
                </p>
              ) : null}

              {(!updater?.error || updater.update?.installMode === "manual") && updater?.update?.available ? (
                <div className="mt-3 flex flex-wrap items-center gap-3 border-t border-border pt-3">
                  <div>
                    <p className="text-sm font-medium text-primary">
                      {t("about.updateAvailable")}
                      {typeof updater.update.size === "number" && updater.update.size > 0
                        ? ` · ${formatBytes(updater.update.size)}`
                        : ""}
                    </p>
                    {updater.update.installMode === "manual" ? (
                      <p className="mt-1 max-w-2xl text-sm text-muted-foreground" data-testid="manual-update-message">
                        {t(updater.update.reason === "signatureKeyMismatch"
                          ? "about.manualSignatureKeyMismatch"
                          : updater.update.reason === "bootstrapRequired"
                            ? "about.manualBootstrapRequired"
                            : "about.manualUpdateRequired")}
                      </p>
                    ) : null}
                  </div>
                  <div className="ml-auto">
                    {updater.update.installMode === "manual" ? (
                      <Button
                        size="sm"
                        variant="primary"
                        data-testid="open-update-release"
                        onClick={() => void openExternal(updater.update!.releasePage)}
                      >
                        <IconExternal />
                        {t("about.openReleasePage")}
                      </Button>
                    ) : (
                      <Button
                        size="sm"
                        variant="primary"
                        data-testid="install-update"
                        disabled={updater.installing}
                        onClick={() => setUpdateDialogOpen(true)}
                      >
                        <IconDownload />
                        {t("about.installAndRestart")}
                      </Button>
                    )}
                  </div>
                </div>
              ) : null}
            </section>

            <div className="p-4 sm:p-5">
              <div className="space-y-3">
                <div>
                  <SectionLabel>{t("about.version")}</SectionLabel>
                  <Mono className="mt-1 text-sm">{about?.app.version ?? "—"}</Mono>
                </div>
                <div>
                  <SectionLabel>{t("about.repository")}</SectionLabel>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="mt-1"
                    onClick={() => void openExternal(PROJECT_REPO)}
                  >
                    <IconExternal />
                    GitHub
                  </Button>
                </div>
                <div>
                  <SectionLabel>{t("about.license")}</SectionLabel>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="mt-1"
                    onClick={() => void openExternal(LICENSE_URL)}
                  >
                    <IconExternal />
                    MIT License
                  </Button>
                </div>
              </div>
            </div>
          </div>
        ) : null}
      </div>

      <ConfirmDialog
        open={updateDialogOpen
          && Boolean(updater?.update?.available)
          && updater?.update?.installMode !== "manual"}
        title={t("about.installAndRestart")}
        description={updater?.update?.latestVersion
          ? `${updater.update.currentVersion} → ${updater.update.latestVersion}`
          : undefined}
        confirmLabel={updater?.installing ? t("about.installing") : t("about.installAndRestart")}
        cancelLabel={t("common.cancel")}
        closeLabel={t("common.close")}
        busy={Boolean(updater?.installing)}
        confirmDisabled={
          !updater?.update?.available
          || updater.update.installMode === "manual"
          || updater.installing
        }
        confirmTestId="confirm-install-update"
        onClose={() => {
          if (!updater?.installing) setUpdateDialogOpen(false);
        }}
        onConfirm={() => void installUpdate()}
      >
        <div className="space-y-3">
          {typeof updater?.update?.size === "number" && updater.update.size > 0 ? (
            <div>
              <SectionLabel>{t("about.size")}</SectionLabel>
              <Mono className="mt-1 text-foreground">{formatBytes(updater.update.size)}</Mono>
            </div>
          ) : null}
          {updater?.update?.notes ? (
            <div>
              <SectionLabel>{t("about.notes")}</SectionLabel>
              <pre className="mt-1 max-h-48 overflow-auto whitespace-pre-wrap rounded-md border border-border bg-muted/40 px-3 py-2 text-[12px] leading-relaxed text-foreground">
                {updater.update.notes}
              </pre>
            </div>
          ) : null}
          {updater?.installing ? (
            <div>
              <div
                className="h-1.5 overflow-hidden rounded-full bg-muted"
                role="progressbar"
                aria-label={t("about.progress")}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.max(0, Math.min(100, updater.progress ?? 0))}
              >
                <div
                  className="h-full bg-primary transition-[width]"
                  style={{ width: `${Math.max(0, Math.min(100, updater.progress ?? 0))}%` }}
                />
              </div>
            </div>
          ) : null}
          {updater?.error ? <ErrorBanner message={updater.error} /> : null}
        </div>
      </ConfirmDialog>

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

function DetailRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="whitespace-nowrap text-muted-foreground">{label}</dt>
      <dd className="min-w-0 break-words text-foreground">{children}</dd>
    </>
  );
}

function OfficialToolRow({
  product,
  busy,
  onPlan,
}: {
  product: OfficialProduct;
  busy: boolean;
  onPlan: (product: OfficialProductId, action: OfficialAction) => void;
}) {
  const { t } = useTranslation();
  const action: OfficialAction = product.installed ? "update" : "install";
  const blocked = !product.available;

  return (
    <article className="rounded-xl border border-border bg-background/35 px-3 py-2.5">
      <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1">
        <ToolLogo tool={product.product} size={20} />
        <h3 className="text-sm font-medium capitalize text-foreground">{product.product}</h3>
        <span
          className={cx(
            "inline-flex items-center rounded border px-1.5 py-0.5 text-[11px] font-medium",
            product.installed
              ? "border-primary/30 bg-primary/10 text-primary"
              : "border-border bg-muted text-muted-foreground",
          )}
        >
          {product.installed ? t("about.installed") : t("about.notInstalled")}
        </span>
        <Mono className="text-foreground">
          {product.currentVersion ?? "—"} → {product.latestVersion ?? "—"}
        </Mono>
        {!blocked ? (
          <Button
            size="sm"
            className="ml-auto"
            disabled={busy}
            data-testid={`official-plan-${product.product}`}
            onClick={() => onPlan(product.product, action)}
          >
            {t("about.planAction")}
          </Button>
        ) : null}
      </div>
      {blocked ? (
        <p className="mt-1.5 flex items-start gap-1.5 text-[12px] text-amber-600 dark:text-amber-500">
          <IconAlert size={12} className="mt-px shrink-0" />
          <span className="min-w-0">
            {product.product === "zcode" ? t("about.zcodeMacOnly") : t("about.officialBlocked")}
            {product.unavailableReason ? ` · ${product.unavailableReason}` : ""}
          </span>
        </p>
      ) : null}
      <Disclosure title={t("common.details")} testId={`official-details-${product.product}`}>
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1.5 text-[12px]">
          <DetailRow label={t("about.executable")}><Mono>{product.executablePath ?? "—"}</Mono></DetailRow>
          <DetailRow label={t("about.source")}>{product.source || "—"}</DetailRow>
          <DetailRow label={t("about.command")}><Mono>{formatArgv(product.argv)}</Mono></DetailRow>
          <DetailRow label={t("about.dest")}><Mono>{product.dest || "—"}</Mono></DetailRow>
        </dl>
      </Disclosure>
    </article>
  );
}
