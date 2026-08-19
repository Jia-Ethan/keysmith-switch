import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { AppShell, type AppPage } from "./components/AppShell";
import { ErrorBanner } from "./components/ErrorBanner";
import { ToastHost } from "./components/ToastHost";
import { useSettings } from "./hooks/useSettings";
import { useToasts } from "./hooks/useToasts";
import { AboutPage } from "./pages/AboutPage";
import { AdvancedPage } from "./pages/AdvancedPage";
import { SettingsPage } from "./pages/SettingsPage";
import { ToolPage } from "./pages/ToolPage";

export function App() {
  const { t } = useTranslation();
  const settingsState = useSettings();
  const toast = useToasts();
  const [page, setPage] = useState<AppPage>({ kind: "tool", tool: "claude" });

  const advancedEnabled = settingsState.settings.advancedToolsEnabled;
  const visiblePage = useMemo<AppPage>(() => {
    if (page.kind === "advanced" && !advancedEnabled) {
      return { kind: "tool", tool: "claude" };
    }
    return page;
  }, [advancedEnabled, page]);

  const rememberProject = async (dir: string) => {
    const dirs = [dir, ...settingsState.settings.recentProjectDirs.filter((item) => item !== dir)].slice(0, 12);
    try {
      await settingsState.save({ recentProjectDirs: dirs });
    } catch {
      settingsState.setSettings({ ...settingsState.settings, recentProjectDirs: dirs });
    }
  };

  return (
    <>
      <AppShell page={visiblePage} onNavigate={setPage} advancedEnabled={advancedEnabled}>
        {settingsState.error ? (
          <div className="mb-3">
            <ErrorBanner message={t("errors.apiUnavailable")} onRetry={() => void settingsState.reload()} retryLabel={t("common.retry")} />
          </div>
        ) : null}
        {visiblePage.kind === "tool" ? (
          <ToolPage
            tool={visiblePage.tool}
            settings={settingsState.settings}
            toast={toast}
            onRememberProject={(dir) => void rememberProject(dir)}
          />
        ) : null}
        {visiblePage.kind === "settings" ? (
          <SettingsPage
            settings={settingsState.settings}
            loadError={settingsState.error}
            onSave={settingsState.save}
            toast={toast}
          />
        ) : null}
        {visiblePage.kind === "about" ? (
          <AboutPage channel={settingsState.settings.updateChannel} toast={toast} />
        ) : null}
        {visiblePage.kind === "advanced" ? (
          <AdvancedPage enabled={advancedEnabled} toast={toast} />
        ) : null}
      </AppShell>
      <ToastHost toasts={toast.toasts} dismiss={toast.dismiss} />
    </>
  );
}

export default App;
