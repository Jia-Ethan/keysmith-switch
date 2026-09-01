import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { AppShell, type AppPage } from "./components/AppShell";
import { DataRecoveryDialog } from "./components/DataRecoveryDialog";
import { ErrorBanner } from "./components/ErrorBanner";
import { FirstRunDialog } from "./components/FirstRunDialog";
import { PromptDetailPage } from "./components/PromptDetailPage";
import { PromptEditPage } from "./components/PromptEditPage";
import { ToastHost } from "./components/ToastHost";
import { UpdateProvider } from "./components/UpdateProvider";
import { useSettings } from "./hooks/useSettings";
import { useTheme } from "./hooks/useTheme";
import { useToasts } from "./hooks/useToasts";
import { isTauriRuntime } from "./lib/runtime";
import { AdvancedPage } from "./pages/AdvancedPage";
import { SettingsPage } from "./pages/SettingsPage";
import { ToolPage } from "./pages/ToolPage";
import * as api from "./api";
import type { FirstRunReport, PromptDetail } from "./types";

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

  useEffect(() => {
    let cancelled = false;
    let unlistenClose: (() => void) | undefined;
    let unlistenQuit: (() => void) | undefined;

    if (isTauriRuntime()) {
      void import("@tauri-apps/api/event").then(({ listen }) => {
        if (cancelled) return;
        void listen("window-close-requested", () => {
          if (dirty && !window.confirm(t("unsaved.leave"))) {
            void api.showMainWindow();
            return;
          }
          void api.hideToTray();
        }).then((unlisten) => {
          if (cancelled) unlisten();
          else unlistenClose = unlisten;
        });
        void listen("app-quit-requested", () => {
          if (dirty && !window.confirm(t("unsaved.leave"))) return;
          void api.quitApp();
        }).then((unlisten) => {
          if (cancelled) unlisten();
          else unlistenQuit = unlisten;
        });
      });
    }

    return () => {
      cancelled = true;
      unlistenClose?.();
      unlistenQuit?.();
    };
  }, [dirty, t]);

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
          <div className="mb-2 shrink-0">
            <ErrorBanner
              message={t("errors.apiUnavailable")}
              onRetry={() => void settingsState.reload()}
              retryLabel={t("common.retry")}
            />
          </div>
        ) : null}
        {visiblePage.kind === "tool" ? (
          <div className="min-h-0 flex-1">
            <ToolPage
              tool={visiblePage.tool}
              settings={settingsState.settings}
              toast={toast}
              onNavigate={navigate}
              onRememberProject={(dir) => void rememberProject(dir)}
              onDirtyChange={setDirty}
              libraryEpoch={libraryEpoch}
            />
          </div>
        ) : null}
        {visiblePage.kind === "prompt-view" ? (
          <PromptDetailPage
            promptId={visiblePage.promptId}
            tool={visiblePage.tool}
            scope={visiblePage.scope}
            projectDir={visiblePage.projectDir}
            toast={toast}
            onClose={() => navigate({ kind: "tool", tool: visiblePage.tool })}
            onEdit={(detail: PromptDetail) =>
              navigate({
                kind: "prompt-edit",
                tool: visiblePage.tool,
                promptId: detail.id,
                creating: false,
                scope: visiblePage.scope,
                projectDir: visiblePage.projectDir,
              })
            }
            onChanged={() => {
              setLibraryEpoch((value) => value + 1);
            }}
          />
        ) : null}
        {visiblePage.kind === "prompt-edit" ? (
          <PromptEditPage
            tool={visiblePage.tool}
            promptId={visiblePage.promptId}
            creating={visiblePage.creating}
            toast={toast}
            onDirtyChange={setDirty}
            onClose={() => {
              setDirty(false);
              setPage({ kind: "tool", tool: visiblePage.tool });
            }}
            onSaved={(id: string) => {
              setDirty(false);
              setLibraryEpoch((value) => value + 1);
              setPage({
                kind: "prompt-view",
                tool: visiblePage.tool,
                promptId: id,
                scope: visiblePage.scope,
                projectDir: visiblePage.projectDir,
              });
            }}
          />
        ) : null}
        {visiblePage.kind === "settings" ? (
          <SettingsPage
            settings={settingsState.settings}
            onSave={settingsState.save}
            onDataChanged={async () => {
              await settingsState.reload();
              setLibraryEpoch((value) => value + 1);
            }}
            toast={toast}
            initialTab={visiblePage.tab}
          />
        ) : null}
        {visiblePage.kind === "advanced" ? <AdvancedPage enabled={advancedEnabled} toast={toast} /> : null}
      </AppShell>
      <ToastHost toasts={toast.toasts} dismiss={toast.dismiss} />
      <FirstRunDialog
        open={Boolean(startup?.firstRun)}
        candidates={startup?.candidates ?? []}
        sidecar={startup?.sidecar ?? null}
        onSkip={() => {
          void api
            .markFirstRunDone()
            .then(() => {
              setStartup((current) =>
                current ? { ...current, firstRun: false, candidates: [] } : current,
              );
            })
            .catch(toast.err);
        }}
        onImport={(paths) => {
          void api
            .importExistingPrompts(paths)
            .then(async (result) => {
              if (result.errors.length) toast.err(result.errors.join("; "));
              await api.markFirstRunDone();
              setLibraryEpoch((value) => value + 1);
              setStartup((current) =>
                current ? { ...current, firstRun: false, candidates: [] } : current,
              );
            })
            .catch(toast.err);
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
