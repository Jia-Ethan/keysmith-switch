import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnnouncementsProvider } from "./components/AnnouncementsProvider";
import { AppShell, type AppPage } from "./components/AppShell";
import { CommandPalette } from "./components/CommandPalette";
import { DataRecoveryDialog } from "./components/DataRecoveryDialog";
import { ErrorBanner } from "./components/ErrorBanner";
import { ExtensionsProvider } from "./components/ExtensionsProvider";
import { FirstRunDialog } from "./components/FirstRunDialog";
import { PromptDetailPage } from "./components/PromptDetailPage";
import { PromptEditPage } from "./components/PromptEditPage";
import { ToastHost } from "./components/ToastHost";
import { UpdateProvider } from "./components/UpdateProvider";
import { useSettings } from "./hooks/useSettings";
import { useTheme } from "./hooks/useTheme";
import { useToasts } from "./hooks/useToasts";
import { getHarnessStatus } from "./lib/harnessState";
import { isTauriRuntime } from "./lib/runtime";
import { AdvancedPage } from "./pages/AdvancedPage";
import { AnnouncementsPage } from "./pages/AnnouncementsPage";
import { ExtensionsPage } from "./pages/ExtensionsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { WorkspacePage } from "./pages/WorkspacePage";
import * as api from "./api";
import type { FirstRunReport, PromptDetail, ToolId } from "./types";

export function App() {
  const { t } = useTranslation();
  useTheme();
  const settingsState = useSettings();
  const toast = useToasts();
  const [page, setPage] = useState<AppPage>({ kind: "tool", tool: "claude" });
  const [dirty, setDirty] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [startup, setStartup] = useState<FirstRunReport | null>(null);
  const [libraryEpoch, setLibraryEpoch] = useState(0);

  const advancedEnabled = settingsState.settings.advancedToolsEnabled;
  const visiblePage = useMemo<AppPage>(() => {
    if (page.kind === "advanced" && !advancedEnabled) {
      return { kind: "tool", tool: "claude" };
    }
    return page;
  }, [advancedEnabled, page]);

  // The accent follows the agent in view, and stays with the last one visited
  // while settings are open, so the chrome never flashes back to a default.
  const [accentTool, setAccentTool] = useState<ToolId>("claude");
  const pageTool = "tool" in visiblePage ? visiblePage.tool : null;
  useEffect(() => {
    if (pageTool) setAccentTool(pageTool);
  }, [pageTool]);
  useEffect(() => {
    document.documentElement.dataset.agent = accentTool;
  }, [accentTool]);

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
          // Closing the window is quitting the app. The only thing that keeps it
          // open is an unsaved draft the person chose not to discard.
          if (dirty && !window.confirm(t("unsaved.leave"))) {
            void api.showMainWindow();
            return;
          }
          void api.quitApp();
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

  const navigate = useCallback(
    (next: AppPage) => {
      if (dirty && !window.confirm(t("unsaved.leave"))) return;
      setPage(next);
    },
    [dirty, t],
  );

  // ⌘K / Ctrl+K toggles the palette from anywhere.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <UpdateProvider
      channel={settingsState.settings.updateChannel}
      autoCheck={settingsState.settings.autoCheckUpdates}
    >
      <ExtensionsProvider
        enabled={settingsState.settings.extensionsEnabled}
        onEnabledChange={async (next) => {
          await settingsState.save({ extensionsEnabled: next });
        }}
      >
        <AnnouncementsProvider>
        <AppShell
          page={visiblePage}
          onNavigate={navigate}
          advancedEnabled={advancedEnabled}
          onOpenPalette={() => setPaletteOpen(true)}
        >
          {settingsState.error ? (
            <div className="shrink-0 px-4 pt-3 sm:px-6">
              <ErrorBanner
                message={t("errors.apiUnavailable")}
                onRetry={() => void settingsState.reload()}
                retryLabel={t("common.retry")}
              />
            </div>
          ) : null}
          {visiblePage.kind === "tool" ? (
            <div key={visiblePage.tool} className="animate-page-in min-h-0 flex-1">
              <WorkspacePage
                key={visiblePage.tool}
                tool={visiblePage.tool}
                settings={settingsState.settings}
                toast={toast}
                onNavigate={navigate}
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
                // Saving does not touch the machine: say so when the edited prompt is the live one.
                if (getHarnessStatus(visiblePage.tool)?.promptId === id) {
                  toast.info(t("prompts.savedRedeploy", { tool: t(`nav.${visiblePage.tool}`) }));
                }
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
            <div className="animate-page-in min-h-0 flex-1 px-4 py-5 sm:px-6">
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
            </div>
          ) : null}
          {visiblePage.kind === "extensions" ? (
            <div className="animate-page-in min-h-0 flex-1">
              <ExtensionsPage toast={toast} />
            </div>
          ) : null}
          {visiblePage.kind === "announcements" ? (
            <div className="animate-page-in min-h-0 flex-1">
              <AnnouncementsPage />
            </div>
          ) : null}
          {visiblePage.kind === "advanced" ? (
            <div className="animate-page-in min-h-0 flex-1 overflow-auto px-4 py-5 sm:px-6">
              <AdvancedPage enabled={advancedEnabled} toast={toast} />
            </div>
          ) : null}
        </AppShell>
        </AnnouncementsProvider>
        <CommandPalette
          open={paletteOpen}
          onClose={() => setPaletteOpen(false)}
          page={visiblePage}
          onNavigate={navigate}
        />
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
      </ExtensionsProvider>
    </UpdateProvider>
  );
}

export default App;
