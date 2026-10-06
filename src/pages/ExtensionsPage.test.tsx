import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ExtensionsProvider } from "../components/ExtensionsProvider";
import type { ExtensionPack, ExtensionsView } from "../types";
import { ExtensionsPage } from "./ExtensionsPage";

const extensionsState = vi.fn();
const extensionsRefresh = vi.fn();
const installExtension = vi.fn();
const previewExtensionRules = vi.fn();
const uninstallExtension = vi.fn();

vi.mock("../api", () => ({
  extensionsState: (...args: unknown[]) => extensionsState(...args),
  extensionsRefresh: (...args: unknown[]) => extensionsRefresh(...args),
  installExtension: (...args: unknown[]) => installExtension(...args),
  previewExtensionRules: (...args: unknown[]) => previewExtensionRules(...args),
  uninstallExtension: (...args: unknown[]) => uninstallExtension(...args),
}));

const base: ExtensionPack = {
  id: "keysmith.example",
  kind: "prompts",
  version: "0.1.0",
  minAppVersion: "0.2.5",
  name: "示例包",
  description: "这段摘要不该出现在卡片上",
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
    // A pack from elsewhere carries no mark at all, rather than a "not official" one.
    expect(screen.getByTestId("extension-third.party")).not.toHaveTextContent("官方");
  });

  it("never draws the pack description on a card, whatever the pack state", async () => {
    extensionsState.mockResolvedValue(
      view([
        base,
        { ...base, id: "installed.pack", name: "已装包", installedVersion: "0.1.0" },
        { ...base, id: "update.pack", name: "可更新包", version: "0.2.0", installedVersion: "0.1.0", updateAvailable: true },
        { ...base, id: "third.party", name: "别处的包", official: false },
        { ...base, id: "future.pack", name: "新版包", compatible: false, minAppVersion: "9.0.0" },
      ]),
    );
    renderPage(true);
    const card = await screen.findByTestId("extension-keysmith.example");
    expect(card).toHaveTextContent("示例包");
    expect(card).toHaveTextContent("v0.1.0");
    for (const id of ["keysmith.example", "installed.pack", "update.pack", "third.party", "future.pack"]) {
      expect(screen.getByTestId(`extension-${id}`)).not.toHaveTextContent(base.description);
    }
    expect(screen.queryByText(base.description)).not.toBeInTheDocument();
  });

  it("installs a pack and says nothing was deployed", async () => {
    installExtension.mockResolvedValue({
      view: view([{ ...base, installedVersion: "0.1.0" }]),
      report: { added: 2, updated: 0, copied: 0, linked: 0, kept: 0, removed: 0 },
    });
    renderPage(true);
    fireEvent.click(await screen.findByTestId("extension-install-keysmith.example"));
    await waitFor(() => expect(installExtension).toHaveBeenCalledWith("keysmith.example", false));
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
    expect(await screen.findByTestId("extension-status-keysmith.example")).toHaveTextContent("v0.1.0 → v0.2.0");
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

  it("asks before turning off, and says installed prompts stay", async () => {
    const { onEnabledChange } = renderPage(true);
    fireEvent.click(await screen.findByTestId("extensions-disable"));
    expect(screen.getByRole("dialog")).toHaveTextContent("已装入提示词库的提示词会保留");
    expect(onEnabledChange).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("extensions-confirm-disable"));
    await waitFor(() => expect(onEnabledChange).toHaveBeenCalledWith(false));
  });

  it("says so when a manual check finds nothing new", async () => {
    extensionsRefresh.mockResolvedValue(view([{ ...base, installedVersion: "0.1.0" }]));
    renderPage(true);
    fireEvent.click(await screen.findByTestId("extensions-refresh"));
    await waitFor(() => expect(toast.ok).toHaveBeenCalledWith("拓展包已全部是最新版本"));
  });

  it("says how many updates a manual check found", async () => {
    const one = { ...base, version: "0.2.0", installedVersion: "0.1.0", updateAvailable: true };
    extensionsRefresh.mockResolvedValue(view([one, { ...one, id: "b" }]));
    renderPage(true);
    fireEvent.click(await screen.findByTestId("extensions-refresh"));
    await waitFor(() => expect(toast.info).toHaveBeenCalledWith("发现 2 个拓展包可更新"));
  });

  it("stays quiet when a manual check fails, and shows the error instead", async () => {
    extensionsRefresh.mockResolvedValue(view([base], { error: "offline" }));
    renderPage(true);
    fireEvent.click(await screen.findByTestId("extensions-refresh"));
    expect(await screen.findByTestId("extensions-error")).toBeInTheDocument();
    expect(toast.ok).not.toHaveBeenCalled();
    expect(toast.info).not.toHaveBeenCalled();
  });

  it("shows when it last looked as a relative time, with the exact time on hover", async () => {
    const checkedAt = new Date(Date.now() - 3 * 60 * 1000).toISOString();
    extensionsState.mockResolvedValue(view([base], { checkedAt }));
    renderPage(true);
    const label = await screen.findByTestId("extensions-checked-at");
    expect(label).toHaveTextContent("3分钟前");
    expect(label.getAttribute("title")).toBeTruthy();
  });

  it("puts packs with an update first", async () => {
    const plain = { ...base, id: "a.plain", installedVersion: "0.1.0" };
    const newer = { ...base, id: "z.newer", version: "0.2.0", installedVersion: "0.1.0", updateAvailable: true };
    extensionsState.mockResolvedValue(view([plain, newer]));
    renderPage(true);
    const list = await screen.findByTestId("extensions-list");
    const ids = Array.from(list.children).map((item) => item.getAttribute("data-testid"));
    expect(ids).toEqual(["extension-z.newer", "extension-a.plain"]);
  });

  it("offers Update all only for two or more, and updates them one by one", async () => {
    const one = { ...base, id: "a", version: "0.2.0", installedVersion: "0.1.0", updateAvailable: true };
    const two = { ...one, id: "b" };
    extensionsState.mockResolvedValue(view([one]));
    const first = renderPage(true);
    await screen.findByTestId("extension-a");
    expect(screen.queryByTestId("extensions-update-all")).not.toBeInTheDocument();
    first.unmount();

    extensionsState.mockResolvedValue(view([one, two]));
    const report = { added: 1, updated: 2, copied: 0, linked: 0, kept: 0, removed: 0 };
    installExtension
      .mockResolvedValueOnce({ view: view([{ ...one, installedVersion: "0.2.0", updateAvailable: false }, two]), report })
      .mockResolvedValueOnce({ view: view([{ ...one, installedVersion: "0.2.0", updateAvailable: false }, { ...two, installedVersion: "0.2.0", updateAvailable: false }]), report });
    renderPage(true);
    const button = await screen.findByTestId("extensions-update-all");
    expect(button).toHaveTextContent("全部更新（2）");
    fireEvent.click(button);
    await waitFor(() => expect(installExtension).toHaveBeenCalledTimes(2));
    expect(installExtension.mock.calls.map((call) => call[0])).toEqual(["a", "b"]);
    await waitFor(() => expect(toast.ok).toHaveBeenCalledWith(expect.stringContaining("已更新 2 个包：更新 4 条，新增 2 条")));
  });

  it("reports a partial Update all when one pack fails", async () => {
    const one = { ...base, id: "a", version: "0.2.0", installedVersion: "0.1.0", updateAvailable: true };
    const two = { ...one, id: "b" };
    extensionsState.mockResolvedValue(view([one, two]));
    installExtension
      .mockResolvedValueOnce({ view: view([{ ...one, installedVersion: "0.2.0", updateAvailable: false }, two]), report: { added: 0, updated: 1, copied: 0, linked: 0, kept: 0, removed: 0 } })
      .mockRejectedValueOnce(new Error("extensions:invalid:bad"));
    renderPage(true);
    fireEvent.click(await screen.findByTestId("extensions-update-all"));
    await waitFor(() => expect(toast.info).toHaveBeenCalledWith("已更新 1/2 个包，其余未更新"));
    expect(toast.err).toHaveBeenCalledWith(expect.stringContaining("不合规"));
  });

  it("shows a rule pack's rules before installing, then installs and turns it on", async () => {
    const rules = { ...base, id: "x.rules", kind: "rules" as const, name: "规则", tools: ["codex" as const] };
    extensionsState.mockResolvedValue(view([rules]));
    previewExtensionRules.mockResolvedValue([{ from: "foo", to: "bar" }]);
    installExtension.mockResolvedValue({
      view: view([{ ...rules, installedVersion: "0.1.0" }]),
      report: { added: 1, updated: 0, copied: 0, linked: 0, kept: 0, removed: 0 },
    });
    renderPage(true);
    const button = await screen.findByTestId("extension-install-x.rules");
    expect(button).toHaveTextContent("查看规则");
    fireEvent.click(button);
    expect(await screen.findByTestId("extensions-rules-preview")).toHaveTextContent("foo");
    expect(installExtension).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("extensions-rules-install-enable"));
    await waitFor(() => expect(installExtension).toHaveBeenCalledWith("x.rules", true));
    await waitFor(() => expect(toast.ok).toHaveBeenCalledWith(expect.stringContaining("输入替换")));
  });

  it("can install a rule pack without turning it on", async () => {
    const rules = { ...base, id: "x.rules", kind: "rules" as const, tools: ["codex" as const] };
    extensionsState.mockResolvedValue(view([rules]));
    previewExtensionRules.mockResolvedValue([{ from: "foo", to: "" }]);
    installExtension.mockResolvedValue({
      view: view([{ ...rules, installedVersion: "0.1.0" }]),
      report: { added: 1, updated: 0, copied: 0, linked: 0, kept: 0, removed: 0 },
    });
    renderPage(true);
    fireEvent.click(await screen.findByTestId("extension-install-x.rules"));
    await screen.findByTestId("extensions-rules-preview");
    fireEvent.click(screen.getByTestId("extensions-rules-install-only"));
    await waitFor(() => expect(installExtension).toHaveBeenCalledWith("x.rules", false));
  });
});
