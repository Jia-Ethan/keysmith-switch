import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { AppShell, type AppPage } from "./components/AppShell";
import { DataRecoveryDialog } from "./components/DataRecoveryDialog";
import { ErrorBanner } from "./components/ErrorBanner";
import { FirstRunDialog } from "./components/FirstRunDialog";
import { ToastHost } from "./components/ToastHost";
import { UpdateProvider } from "./components/UpdateProvider";
import { useSettings } from "./hooks/useSettings";
import { useTheme } from "./hooks/useTheme";
import { useToasts } from "./hooks/useToasts";
import { AdvancedPage } from "./pages/AdvancedPage";
import { SettingsPage } from "./pages/SettingsPage";
import { ToolPage } from "./pages/ToolPage";
import * as api from "./api";
import type { FirstRunReport } from "./types";
import { useEffect } from "react";

export function App() {
  const { t } = useTranslation();
  useTheme();
  const settingsState = useSettings();
  const toast = useToasts();
  const [page, setPage] = useState<AppPage>({ kind: "tool", tool: "claude" });
  const [dirty, setDirty] = useState(false);
  const [startup, setStartup] = useState<FirstRunReport | null>(null);
  const [libraryEpoch, setLibraryEpoch] = useState(0);

  const advancedEnabled = settingsState.settings.advancedToolsEnabled;
  const visiblePage = useMemo<AppPage>(() => {
    if (page.kind === "advanced" && !advancedEnabled) {
      return { kind: "tool", tool: "claude" };
    }
    return page;
  }, [advancedEnabled, page]);

  useEffect(() => {
    void api
      .getStartupReport()
      .then(setStartup)
      .catch(() => setStartup(null));
  }, []);

  const rememberProject = async (dir: string) => {
    const dirs = [dir, ...settingsState.settings.recentProjectDirs.filter((item) => item !== dir)].slice(
      0,
      12,
    );
    try {
      await settingsState.save({ recentProjectDirs: dirs });
    } catch {
      settingsState.setSettings({ ...settingsState.settings, recentProjectDirs: dirs });
    }
  };

  const navigate = useCallback(
    (next: AppPage) => {
      if (dirty && !window.confirm(t("unsaved.leave"))) return;
      setPage(next);
    },
    [dirty, t],
  );

  return (
    <UpdateProvider
      channel={settingsState.settings.updateChannel}
      autoCheck={settingsState.settings.autoCheckUpdates}
    >
      <AppShell page={visiblePage} onNavigate={navigate} advancedEnabled={advancedEnabled}>
        {settingsState.error ? (
          <div className="mb-2">
            <ErrorBanner
              message={t("errors.apiUnavailable")}
              onRetry={() => void settingsState.reload()}
              retryLabel={t("common.retry")}
            />
          </div>
        ) : null}
        {visiblePage.kind === "tool" ? (
          <ToolPage
            tool={visiblePage.tool}
            settings={settingsState.settings}
            toast={toast}
            onRememberProject={(dir) => void rememberProject(dir)}
            onDirtyChange={setDirty}
            libraryEpoch={libraryEpoch}
          />
        ) : null}
        {visiblePage.kind === "settings" ? (
          <SettingsPage
            settings={settingsState.settings}
            loadError={settingsState.error}
            onSave={settingsState.save}
            toast={toast}
            initialTab={visiblePage.tab}
          />
        ) : null}
        {visiblePage.kind === "advanced" ? <AdvancedPage enabled={advancedEnabled} toast={toast} /> : null}
      </AppShell>
      <ToastHost toasts={toast.toasts} dismiss={toast.dismiss} />
      <FirstRunDialog
        open={Boolean(startup?.firstRun && startup.candidates.length)}
        candidates={startup?.candidates ?? []}
        onSkip={() => {
          void api.markFirstRunDone();
          setStartup((current) =>
            current ? { ...current, firstRun: false, candidates: [] } : current,
          );
        }}
        onImport={(paths) => {
          void api.importExistingPrompts(paths).then(() => {
            void api.markFirstRunDone();
            setLibraryEpoch((value) => value + 1);
            setStartup((current) =>
              current ? { ...current, firstRun: false, candidates: [] } : current,
            );
          });
        }}
      />
      <DataRecoveryDialog
        marker={startup?.recovery ?? null}
        onAck={() => {
          void api.acknowledgeRecovery();
          setStartup((current) => (current ? { ...current, recovery: null } : current));
        }}
      />
    </UpdateProvider>
  );
}

export default App;
