import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppPage } from "./components/AppShell";

const eventHandlers = new Map<string, () => void>();
const hideToTray = vi.fn();
const showMainWindow = vi.fn();
const quitApp = vi.fn();
let callbackId = 0;
const callbacks = new Map<number, (event: unknown) => void>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: () => void) => {
    eventHandlers.set(name, handler);
    return () => eventHandlers.delete(name);
  }),
}));

vi.mock("./api", () => ({
  getStartupReport: vi.fn().mockResolvedValue({
    firstRun: false,
    candidates: [],
    recovery: null,
    sidecar: { pythonRequired: false, tools: [] },
  }),
  hideToTray: (...args: unknown[]) => hideToTray(...args),
  showMainWindow: (...args: unknown[]) => showMainWindow(...args),
  quitApp: (...args: unknown[]) => quitApp(...args),
  checkAppUpdate: vi.fn(),
  logFrontendError: vi.fn(),
}));

vi.mock("./hooks/useSettings", () => ({
  useSettings: () => ({
    settings: {
      advancedToolsEnabled: false,
      recentProjectDirs: [],
      updateChannel: "stable",
      autoCheckUpdates: false,
    },
    error: null,
    save: vi.fn(),
    reload: vi.fn(),
    setSettings: vi.fn(),
  }),
}));

vi.mock("./hooks/useTheme", () => ({ useTheme: vi.fn() }));
vi.mock("./pages/ToolPage", () => ({
  ToolPage: ({ onDirtyChange }: { onDirtyChange: (dirty: boolean) => void }) => (
    <button type="button" onClick={() => onDirtyChange(true)}>make dirty</button>
  ),
}));
vi.mock("./components/AppShell", () => ({
  AppShell: ({ children }: { page: AppPage; children: React.ReactNode }) => <div>{children}</div>,
}));

describe("desktop lifecycle guards", () => {
  beforeEach(() => {
    eventHandlers.clear();
    hideToTray.mockReset().mockResolvedValue({ ok: true });
    showMainWindow.mockReset().mockResolvedValue({ ok: true });
    quitApp.mockReset().mockResolvedValue({ ok: true });
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {
        transformCallback: (callback: (event: unknown) => void) => {
          callbackId += 1;
          callbacks.set(callbackId, callback);
          return callbackId;
        },
        unregisterCallback: (id: number) => callbacks.delete(id),
        invoke: async (command: string, args: { event?: string; handler?: number }) => {
          if (command === "plugin:event|listen" && args.event && args.handler) {
            const callback = callbacks.get(args.handler);
            if (callback) eventHandlers.set(args.event, () => callback({ event: args.event, payload: null }));
          }
          return 1;
        },
      },
    });
    Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
      configurable: true,
      value: { unregisterListener: vi.fn() },
    });
  });

  it("restores a close-to-tray window when a dirty draft is kept", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const { App } = await import("./App");
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "make dirty" }));
    await waitFor(() => expect(eventHandlers.has("window-close-requested")).toBe(true));
    act(() => eventHandlers.get("window-close-requested")?.());
    expect(confirm).toHaveBeenCalledTimes(1);
    expect(showMainWindow).toHaveBeenCalledTimes(1);
    expect(hideToTray).not.toHaveBeenCalled();
    confirm.mockRestore();
  });

  it("quits only after a dirty draft is confirmed", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { App } = await import("./App");
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "make dirty" }));
    await waitFor(() => expect(eventHandlers.has("app-quit-requested")).toBe(true));
    act(() => eventHandlers.get("app-quit-requested")?.());
    expect(confirm).toHaveBeenCalledTimes(1);
    expect(quitApp).toHaveBeenCalledTimes(1);
    confirm.mockRestore();
  });
});
