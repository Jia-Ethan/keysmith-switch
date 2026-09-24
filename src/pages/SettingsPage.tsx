import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ErrorBanner } from "../components/ErrorBanner";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { Feedback } from "../components/Feedback";
import { useUpdateOptional } from "../components/UpdateProvider";
import { IconDownload, IconExternal, IconMonitor, IconMoon, IconRefresh, IconSun } from "../components/icons";
import { ToolLogo } from "../components/ToolLogos";
import { Button, Checkbox, Mono, Segmented, Select, SettingRow, SectionLabel, cx } from "../components/ui";
import { useTheme, type ThemeMode } from "../hooks/useTheme";
import type { ToastApi } from "../hooks/useToasts";
import { formatBytes } from "../lib/format";
import { openExternal } from "../lib/runtime";
import type { AboutInfo, Language, Settings, SettingsPatch, ToolId } from "../types";
import { PUBLIC_RELEASE_PAGE } from "../types";

type TabId = "general" | "tools" | "about";

const TABS: TabId[] = ["general", "tools", "about"];

const KEYSMITHS: { tool: ToolId; name: string; repo: string }[] = [
  { tool: "claude", name: "Claude Keysmith", repo: "https://github.com/Jia-Ethan/claude-keysmith" },
  { tool: "codex", name: "Codex Keysmith", repo: "https://github.com/Jia-Ethan/codex-keysmith" },
  { tool: "grok", name: "Grok Keysmith", repo: "https://github.com/Jia-Ethan/grok-keysmith" },
  { tool: "zcode", name: "Zcode Keysmith", repo: "https://github.com/Jia-Ethan/zcode-keysmith" },
];

export function SettingsPage({
  settings,
  onSave,
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
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const updateInstallPending = useRef(false);

  useEffect(() => {
    if (TABS.includes(initialTab as TabId)) setTab(initialTab as TabId);
    else if (initialTab === "data") setTab("general");
  }, [initialTab]);

  useEffect(() => {
    setUpdateDialogOpen(false);
  }, [updater?.update?.latestVersion]);

  useEffect(() => {
    let cancelled = false;
    void api
      .getAbout()
      .then((info) => {
        if (!cancelled) setAbout(info);
      })
      .catch(() => {
        if (!cancelled) setAbout(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

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
        className="flex shrink-0 gap-1 overflow-x-auto rounded-2xl border border-border bg-card p-1.5"
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
                "h-10 shrink-0 rounded-xl px-3 text-[15px] font-medium transition-colors",
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
        className="min-h-0 flex-1 overflow-auto rounded-2xl border border-border bg-card shadow-[0_16px_50px_hsl(var(--foreground)/0.04)]"
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
                    { value: "light" as ThemeMode, label: <><IconSun size={14} data-testid="theme-sun" />{t("settings.themeLight")}</> },
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
          </div>
        ) : null}

        {tab === "tools" ? (
          <div className="flex flex-col gap-2 p-4 sm:p-5" data-testid="settings-keysmiths">
            {KEYSMITHS.map((item) => (
              <article
                key={item.tool}
                data-testid={`keysmith-${item.tool}`}
                className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-2xl border border-border bg-background/35 px-3 py-2.5"
              >
                <ToolLogo tool={item.tool} size={20} />
                <h2 className="min-w-[9rem] text-[15px] font-medium text-foreground">{item.name}</h2>
                <Button
                  size="sm"
                  variant="ghost"
                  className="ml-auto"
                  data-testid={`keysmith-repo-${item.tool}`}
                  onClick={() => void openExternal(item.repo)}
                >
                  <IconExternal />
                  {item.repo.replace("https://github.com/", "")}
                </Button>
              </article>
            ))}
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
                <div className="mt-3 space-y-2" key={`error-${updater.checkCount}`}>
                  <ErrorBanner
                    message={updater.error}
                    onRetry={() => void updater.check()}
                    retryLabel={t("common.retry")}
                  />
                  <Button
                    size="sm"
                    variant="outline"
                    data-testid="open-update-release-on-error"
                    onClick={() => void openExternal(updater.update?.releasePage || PUBLIC_RELEASE_PAGE)}
                  >
                    <IconExternal />
                    {t("about.openReleasePage")}
                  </Button>
                </div>
              ) : null}

              {(!updater?.error || updater.update?.installMode === "manual") && !updater?.checking && updater?.update && !updater.update.available ? (
                <p className="mt-3 animate-page-in text-sm text-primary" role="status" key={`current-${updater.checkCount}`}>
                  {updater.update.currentVersion} · {t("about.upToDate")}
                </p>
              ) : null}

              {(!updater?.error || updater.update?.installMode === "manual") && !updater?.checking && updater?.update?.available ? (
                <div className="mt-3 flex animate-page-in flex-wrap items-center gap-3 border-t border-border pt-3" key={`available-${updater.checkCount}`}>
                  <div>
                    <p className="text-sm font-medium text-primary">
                      {t("about.updateAvailable")} · {updater.update.latestVersion ?? "—"}
                      {typeof updater.update.size === "number" && updater.update.size > 0
                        ? ` · ${formatBytes(updater.update.size)}`
                        : ""}
                    </p>
                    {updater.update.installMode === "manual" ? (
                      <div className="mt-1 max-w-2xl">
                        <p className="text-sm text-muted-foreground" data-testid="manual-update-message">
                          {t(updater.update.reason === "signatureKeyMismatch"
                            ? "about.manualSignatureKeyMismatch"
                            : updater.update.reason === "bootstrapRequired"
                              ? "about.manualBootstrapRequired"
                              : "about.manualUpdateRequired")}
                        </p>
                        {updater.update.detail?.message ? (
                          <details className="mt-2" data-testid="update-error-details">
                            <summary className="cursor-pointer text-xs text-muted-foreground">
                              {t("about.updateDetails")}
                            </summary>
                            <pre className="mt-1 max-h-32 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-border bg-muted/40 px-2.5 py-1.5 font-mono text-[11px] leading-snug text-muted-foreground">
                              {updater.update.detail.message}
                            </pre>
                          </details>
                        ) : null}
                      </div>
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

            <Feedback />

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
              <pre className="mt-1 max-h-48 overflow-auto whitespace-pre-wrap rounded-xl border border-border bg-muted/40 px-3 py-2 text-[13px] leading-snug text-foreground">
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
    </div>
  );
}
