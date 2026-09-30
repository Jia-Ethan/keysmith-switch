import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ExtensionsProvider } from "../components/ExtensionsProvider";
import type { ExtensionPack, ExtensionsView } from "../types";
import { ExtensionsPage } from "./ExtensionsPage";

const extensionsState = vi.fn();
const extensionsRefresh = vi.fn();
const installExtension = vi.fn();
const uninstallExtension = vi.fn();

vi.mock("../api", () => ({
  extensionsState: (...args: unknown[]) => extensionsState(...args),
  extensionsRefresh: (...args: unknown[]) => extensionsRefresh(...args),
  installExtension: (...args: unknown[]) => installExtension(...args),
  uninstallExtension: (...args: unknown[]) => uninstallExtension(...args),
}));

const base: ExtensionPack = {
  id: "keysmith.example",
  version: "0.1.0",
  minAppVersion: "0.2.5",
  name: "示例包",
  description: "演示",
  tools: ["claude", "codex"],
  itemCount: 2,
  size: 1312,
  official: true,
  compatible: true,
  installedVersion: null,
  updateAvailable: false,
};
const view = (packs: ExtensionPack[], over: Partial<ExtensionsView> = {}): ExtensionsView => ({
  packs,
  updates: packs.filter((p) => p.updateAvailable).length,
  error: null,
  checkedAt: null,
  ...over,
});

const toast = { ok: vi.fn(), err: vi.fn(), info: vi.fn(), toasts: [], dismiss: vi.fn() };

function renderPage(enabled: boolean, onEnabledChange = vi.fn().mockResolvedValue(undefined)) {
  const ui = (on: boolean) => (
    <ExtensionsProvider enabled={on} onEnabledChange={onEnabledChange}>
      <ExtensionsPage toast={toast as never} />
    </ExtensionsProvider>
  );
  const utils = render(ui(enabled));
  return { ...utils, onEnabledChange, again: (on: boolean) => utils.rerender(ui(on)) };
}

describe("ExtensionsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    extensionsState.mockResolvedValue(view([base]));
    extensionsRefresh.mockResolvedValue(view([base]));
  });

  it("explains itself and reads nothing while off", () => {
    renderPage(false);
    expect(screen.getByTestId("extensions-off")).toHaveTextContent("不会上传你的任何内容");
    expect(screen.getByTestId("extensions-off")).toHaveTextContent("不会自动部署");
    expect(extensionsState).not.toHaveBeenCalled();
    expect(extensionsRefresh).not.toHaveBeenCalled();
  });

  it("asks the caller to switch on, then looks", async () => {
    const { onEnabledChange, again } = renderPage(false);
    fireEvent.click(screen.getByTestId("extensions-enable"));
    await waitFor(() => expect(onEnabledChange).toHaveBeenCalledWith(true));
    again(true);
    expect(await screen.findByTestId("extension-keysmith.example")).toBeInTheDocument();
  });

  it("lists packs with the official mark from the backend, not from anything in the pack", async () => {
    extensionsState.mockResolvedValue(view([base, { ...base, id: "third.party", name: "别处的包", official: false }]));
    renderPage(true);
    expect(await screen.findByTestId("extension-official-keysmith.example")).toHaveTextContent("官方");
    expect(screen.queryByTestId("extension-official-third.party")).not.toBeInTheDocument();
    expect(screen.getByTestId("extension-third.party")).toHaveTextContent("非官方");
  });

  it("installs a pack and says nothing was deployed", async () => {
    installExtension.mockResolvedValue({
      view: view([{ ...base, installedVersion: "0.1.0" }]),
      report: { added: 2, updated: 0, copied: 0, linked: 0, kept: 0, removed: 0 },
    });
    renderPage(true);
    fireEvent.click(await screen.findByTestId("extension-install-keysmith.example"));
    await waitFor(() => expect(installExtension).toHaveBeenCalledWith("keysmith.example"));
    await waitFor(() => expect(toast.ok).toHaveBeenCalledWith(expect.stringContaining("未部署")));
    expect(await screen.findByTestId("extension-status-keysmith.example")).toHaveTextContent("已安装 v0.1.0");
    expect(screen.queryByTestId("extension-install-keysmith.example")).not.toBeInTheDocument();
  });

  it("offers an update, and reports edits that were kept aside", async () => {
    const installed = { ...base, version: "0.2.0", installedVersion: "0.1.0", updateAvailable: true };
    extensionsState.mockResolvedValue(view([installed]));
    installExtension.mockResolvedValue({
      view: view([{ ...installed, installedVersion: "0.2.0", updateAvailable: false }]),
      report: { added: 1, updated: 0, copied: 1, linked: 0, kept: 1, removed: 0 },
    });
    renderPage(true);
    expect(await screen.findByTestId("extension-status-keysmith.example")).toHaveTextContent("可更新到 v0.2.0");
    fireEvent.click(screen.getByTestId("extension-install-keysmith.example"));
    await waitFor(() => expect(toast.ok).toHaveBeenCalledWith(expect.stringContaining("另存 1 条")));
  });

  it("lists a pack that needs a newer app but cannot install it", async () => {
    extensionsState.mockResolvedValue(view([{ ...base, compatible: false, minAppVersion: "9.0.0" }]));
    renderPage(true);
    expect(await screen.findByTestId("extension-status-keysmith.example")).toHaveTextContent("需要 App 9.0.0");
    expect(screen.queryByTestId("extension-install-keysmith.example")).not.toBeInTheDocument();
  });

  it("confirms before removing, and says what stays", async () => {
    extensionsState.mockResolvedValue(view([{ ...base, installedVersion: "0.1.0" }]));
    uninstallExtension.mockResolvedValue({
      view: view([base]),
      report: { added: 0, updated: 0, copied: 0, linked: 0, kept: 1, removed: 1 },
    });
    renderPage(true);
    fireEvent.click(await screen.findByTestId("extension-uninstall-keysmith.example"));
    expect(screen.getByRole("dialog")).toHaveTextContent("你改过的和正在使用的会保留");
    expect(uninstallExtension).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("extensions-confirm-uninstall"));
    await waitFor(() => expect(uninstallExtension).toHaveBeenCalledWith("keysmith.example"));
    await waitFor(() => expect(toast.ok).toHaveBeenCalledWith(expect.stringContaining("移除 1 条")));
  });

  it("shows why a look failed, and keeps the packs it already had", async () => {
    extensionsState.mockResolvedValue(view([base]));
    extensionsRefresh.mockResolvedValue(view([base], { error: "too-new" }));
    renderPage(true);
    fireEvent.click(await screen.findByTestId("extensions-refresh"));
    expect(await screen.findByTestId("extensions-error")).toHaveTextContent("需要更新 App");
    expect(screen.getByTestId("extension-keysmith.example")).toBeInTheDocument();
  });

  it("shows a friendly error and no change when an install fails", async () => {
    installExtension.mockRejectedValue(new Error("extensions:invalid:archive hash differs"));
    renderPage(true);
    fireEvent.click(await screen.findByTestId("extension-install-keysmith.example"));
    await waitFor(() => expect(toast.err).toHaveBeenCalledWith(expect.stringContaining("不合规")));
    expect(screen.getByTestId("extension-install-keysmith.example")).toBeInTheDocument();
  });

  it("can be turned off again", async () => {
    const { onEnabledChange } = renderPage(true);
    fireEvent.click(await screen.findByTestId("extensions-disable"));
    await waitFor(() => expect(onEnabledChange).toHaveBeenCalledWith(false));
  });
});
