import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import * as api from "../api";
import { ErrorBanner } from "../components/ErrorBanner";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { Feedback } from "../components/Feedback";
import { UpdateSection } from "../components/UpdateSection";
import { VersionsTab } from "../components/VersionsTab";
import { useUpdateOptional } from "../components/UpdateProvider";
import { IconCheck, IconExternal, IconMonitor, IconMoon, IconSun } from "../components/icons";
import { ToolLogo } from "../components/ToolLogos";
import { Button, Mono, NoticeDot, Segmented, SettingRow, SectionLabel, cx, useSlidingIndicator } from "../components/ui";
import keysmithIcon from "../assets/keysmith-icon.png";
import { Dropdown } from "../components/Dropdown";
import { useTheme, type ThemeMode } from "../hooks/useTheme";
import type { ToastApi } from "../hooks/useToasts";
import { formatBytes } from "../lib/format";
import { openExternal } from "../lib/runtime";
import type { AboutInfo, Language, Settings, SettingsPatch, ToolId } from "../types";

type TabId = "general" | "tools" | "versions" | "about";

const TABS: TabId[] = ["general", "tools", "versions", "about"];

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
  const [saved, setSaved] = useState<number | null>(null);
  const savedCount = useRef(0);
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

  // "Saved" is a quiet acknowledgement beside the title that fades on its own.
  useEffect(() => {
    if (saved === null) return undefined;
    const timer = window.setTimeout(() => setSaved(null), 1800);
    return () => window.clearTimeout(timer);
  }, [saved]);

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
      savedCount.current += 1;
      setSaved(savedCount.current);
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
        <div className="flex min-w-0 items-center gap-3">
          <h1 className="text-[20px] font-semibold tracking-[-0.02em] text-foreground">{t("settings.title")}</h1>
          {saved !== null ? (
            <span
              key={saved}
              role="status"
              data-testid="settings-saved"
              className="animate-toast-in inline-flex h-6 items-center gap-1 rounded-full bg-success/15 px-2.5 text-[12px] font-medium text-success"
            >
              <IconCheck size={12} />
              {t("settings.saved")}
            </span>
          ) : null}
        </div>
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
              {item === "about" && updater?.hasUpdate ? <NoticeDot className="right-1 top-1" /> : null}
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

        {tab === "versions" ? <VersionsTab toast={toast} /> : null}

        {tab === "about" ? (
          <div className="space-y-4">
            <section className="surface-card relative overflow-hidden px-5 py-6" data-testid="about-hero">
              <div
                aria-hidden="true"
                className="pointer-events-none absolute inset-0 bg-[radial-gradient(70%_120%_at_50%_0%,rgb(var(--primary)/0.14),transparent_70%)]"
              />
              <div className="relative flex flex-col items-center text-center">
                <img src={keysmithIcon} alt="" className="brand-mark-static h-[72px] w-[72px]" aria-hidden="true" draggable={false} />
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
                    PolyForm Noncommercial
                  </Button>
                </div>
              </div>
            </section>
          <div className="surface-card overflow-hidden">
            <UpdateSection updater={updater} fallbackVersion={about?.app.version} onInstall={() => setUpdateDialogOpen(true)} />

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
