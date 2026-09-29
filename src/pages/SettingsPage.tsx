import { Fragment, useEffect, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ErrorBanner } from "../components/ErrorBanner";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { Feedback } from "../components/Feedback";
import { useUpdateOptional } from "../components/UpdateProvider";
import { IconDownload, IconExternal, IconMonitor, IconMoon, IconRefresh, IconSun } from "../components/icons";
import { ToolLogo } from "../components/ToolLogos";
import { Button, Mono, Segmented, SettingRow, SectionLabel, cx, useSlidingIndicator } from "../components/ui";
import keysmithIcon from "../assets/keysmith-icon.png";
import { modLabel, shortcutLabel } from "../lib/platform";
import { Dropdown } from "../components/Dropdown";
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
  const tabsRef = useRef<HTMLDivElement>(null);
  const tabIndicator = useSlidingIndicator(tabsRef, '[aria-selected="true"]', [tab]);

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
    <div className="mx-auto flex h-full min-h-0 w-full max-w-3xl flex-col gap-4">
      <div className="flex shrink-0 items-center justify-between gap-3">
        <h1 className="text-[20px] font-semibold tracking-[-0.02em] text-foreground">{t("settings.title")}</h1>
      <div
        ref={tabsRef}
        className="relative flex shrink-0 gap-0.5 overflow-x-auto rounded-xl border border-border/80 bg-muted/70 p-[3px]"
        role="tablist"
        aria-label={t("settings.title")}
      >
        {tabIndicator ? (
          <span
            aria-hidden="true"
            className="pointer-events-none absolute rounded-[9px] bg-card shadow-[0_1px_2px_hsl(var(--shadow)/0.12)] transition-[left,width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] dark:bg-accent"
            style={{ left: tabIndicator.left, width: tabIndicator.width, top: tabIndicator.top, height: tabIndicator.height }}
          />
        ) : null}
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
                "relative z-[1] h-8 shrink-0 rounded-[9px] px-3.5 text-[13px] font-medium transition-colors",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                active
                  ? cx("text-foreground", !tabIndicator && "bg-card shadow-sm")
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {t(`settings.tab.${item}`)}
            </button>
          );
        })}
      </div>
      </div>

      <div
        id={`settings-panel-${tab}`}
        key={tab}
        role="tabpanel"
        className="animate-page-in min-h-0 flex-1 overflow-auto pb-6"
        aria-busy={busy || undefined}
      >
        {tab === "general" ? (
          <div className="space-y-4">
          <div className="surface-card overflow-hidden">
            <SettingRow
              label={t("settings.language")}
              control={
                <Dropdown<Language>
                  label={t("settings.language")}
                  testId="settings-language"
                  menuTestId="settings-language-menu"
                  optionTestId={(value) => `settings-language-${value}`}
                  className="w-[11.5rem]"
                  value={settings.language}
                  disabled={busy}
                  onChange={(value) => void patch({ language: value })}
                  options={[
                    { value: "zh-CN", label: t("settings.languageZhCN") },
                    { value: "zh-TW", label: t("settings.languageZhTW") },
                    { value: "en", label: t("settings.languageEn") },
                  ]}
                />
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
          </div>
          <ShortcutList />
          </div>
        ) : null}

        {tab === "tools" ? (
          <div className="surface-card flex flex-col gap-2 p-4 sm:p-5" data-testid="settings-keysmiths">
            {KEYSMITHS.map((item) => (
              <article
                key={item.tool}
                data-testid={`keysmith-${item.tool}`}
                className="group flex flex-wrap items-center gap-x-3 gap-y-1 rounded-xl border border-border bg-background/50 px-3 py-2.5 transition-colors hover:border-foreground/15"
              >
                <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-card ring-1 ring-border">
                  <ToolLogo tool={item.tool} size={20} />
                </span>
                <h2 className="min-w-[9rem] text-[13.5px] font-medium text-foreground">{item.name}</h2>
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
          <div className="space-y-4">
            <section className="surface-card relative overflow-hidden px-5 py-6" data-testid="about-hero">
              <div
                aria-hidden="true"
                className="pointer-events-none absolute inset-0 bg-[radial-gradient(70%_120%_at_50%_0%,rgb(var(--primary)/0.14),transparent_70%)]"
              />
              <div className="relative flex flex-col items-center text-center">
                <span className="flex h-16 w-16 items-center justify-center rounded-[20px] bg-card shadow-glow ring-1 ring-border">
                  <img src={keysmithIcon} alt="" className="h-11 w-11" aria-hidden="true" draggable={false} />
                </span>
                <h2 className="mt-3 text-[18px] font-semibold tracking-[-0.01em] text-foreground">{t("app.name")}</h2>
                <p className="mt-0.5 text-[12.5px] text-muted-foreground">{t("about.tagline")}</p>
                <span className="mt-2 rounded-full bg-muted px-2.5 py-0.5 font-mono text-[11.5px] text-muted-foreground">
                  v{about?.app.version ?? updater?.update?.currentVersion ?? "—"}
                </span>
                <div className="mt-3 flex flex-wrap justify-center gap-1.5">
                  <Button size="sm" variant="outline" onClick={() => void openExternal(PROJECT_REPO)}>
                    <IconExternal />
                    GitHub
                  </Button>
                  <Button size="sm" variant="outline" onClick={() => void openExternal(LICENSE_URL)}>
                    <IconExternal />
                    MIT License
                  </Button>
                </div>
              </div>
            </section>
          <div className="surface-card overflow-hidden">
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
                            <pre className="mt-1 max-h-32 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-border bg-muted/40 px-2.5 py-1.5 font-mono text-[12px] leading-snug text-muted-foreground">
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
              <pre className="mt-1 max-h-48 overflow-auto whitespace-pre-wrap rounded-lg border border-border bg-muted/40 px-3 py-2 text-[13px] leading-snug text-foreground">
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

function ShortcutList() {
  const { t } = useTranslation();
  const rows: Array<{ label: string; keys: string[] }> = [
    { label: t("shortcuts.switchAgent"), keys: [`${shortcutLabel("1")} – ${shortcutLabel("4")}`] },
    { label: t("shortcuts.compose"), keys: [shortcutLabel("n")] },
    { label: t("shortcuts.preview"), keys: [shortcutLabel("Enter")] },
    { label: t("shortcuts.search"), keys: ["/", shortcutLabel("f")] },
    { label: t("shortcuts.refresh"), keys: [shortcutLabel("r")] },
    { label: t("shortcuts.settings"), keys: [shortcutLabel(",")] },
  ];
  return (
    <section className="surface-card overflow-hidden" data-testid="settings-shortcuts" aria-label={t("shortcuts.title")}>
      <div className="flex items-center justify-between border-b border-border/70 px-5 py-3">
        <SectionLabel>{t("shortcuts.title")}</SectionLabel>
        <span className="text-[11.5px] text-muted-foreground">{modLabel()}</span>
      </div>
      <dl className="divide-y divide-border/70">
        {rows.map((row) => (
          <div key={row.label} className="flex items-center justify-between gap-3 px-5 py-2.5 text-[13px]">
            <dt className="text-foreground">{row.label}</dt>
            <dd className="flex items-center gap-1.5">
              {row.keys.map((key, index) => (
                <Fragment key={key}>
                  {index > 0 ? <span className="text-[11px] text-muted-foreground">/</span> : null}
                  <kbd className="rounded-md border border-border bg-muted px-1.5 py-0.5 font-sans text-[11.5px] font-medium text-muted-foreground shadow-[0_1px_0_hsl(var(--border))]">
                    {key}
                  </kbd>
                </Fragment>
              ))}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
