import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n";
import type { ToastApi } from "../hooks/useToasts";
import { DEFAULT_SETTINGS, type UpdateCheck } from "../types";
import { SettingsPage } from "./SettingsPage";

const listTools = vi.fn();
const getDataDirs = vi.fn();
const listBackups = vi.fn();
const restoreBackup = vi.fn();
const createBackup = vi.fn();
const inspectZipArchive = vi.fn();
const planClearAllData = vi.fn();
const clearAllData = vi.fn();
const getAbout = vi.fn();
const planOfficialAction = vi.fn();
const confirmOfficialAction = vi.fn();
const cancelOfficialAction = vi.fn();
const checkUpdate = vi.fn();
const installUpdate = vi.fn();
const openExternal = vi.fn();
let updaterState: {
  update: UpdateCheck | null;
  checking: boolean;
  installing: boolean;
  progress: number | null;
  error: string | null;
  check: typeof checkUpdate;
  install: typeof installUpdate;
};

vi.mock("../api", () => ({
  listTools: (...args: unknown[]) => listTools(...args),
  getDataDirs: (...args: unknown[]) => getDataDirs(...args),
  listBackups: (...args: unknown[]) => listBackups(...args),
  restoreBackup: (...args: unknown[]) => restoreBackup(...args),
  createBackup: (...args: unknown[]) => createBackup(...args),
  inspectZipArchive: (...args: unknown[]) => inspectZipArchive(...args),
  importMarkdownFiles: vi.fn(),
  importZipArchive: vi.fn(),
  exportZipArchive: vi.fn(),
  planClearAllData: (...args: unknown[]) => planClearAllData(...args),
  clearAllData: (...args: unknown[]) => clearAllData(...args),
  getAbout: (...args: unknown[]) => getAbout(...args),
  planOfficialAction: (...args: unknown[]) => planOfficialAction(...args),
  confirmOfficialAction: (...args: unknown[]) => confirmOfficialAction(...args),
  cancelOfficialAction: (...args: unknown[]) => cancelOfficialAction(...args),
}));

vi.mock("../lib/runtime", () => ({
  openExternal: (...args: unknown[]) => openExternal(...args),
  pickFiles: vi.fn().mockResolvedValue([]),
  pickSavePath: vi.fn().mockResolvedValue(null),
  isTauriRuntime: vi.fn().mockReturnValue(false),
}));

vi.mock("../components/UpdateProvider", () => ({
  useUpdateOptional: () => updaterState,
}));

describe("SettingsPage data safety", () => {
  const toast = {
    toasts: [],
    dismiss: vi.fn(),
    info: vi.fn(),
    ok: vi.fn(),
    err: vi.fn(),
  } as unknown as ToastApi;

  beforeEach(() => {
    applyLanguage("zh-CN");
    vi.clearAllMocks();
    listTools.mockResolvedValue({ tools: [] });
    getAbout.mockResolvedValue({
      app: {
        name: "Keysmith Switch",
        version: "0.1.1",
        channel: "stable",
        preview: true,
        signed: false,
        identifier: "com.jia-ethan.keysmith-switch",
        website: "https://github.com/Jia-Ethan/keysmith-switch",
        github: "https://github.com/Jia-Ethan/keysmith-switch",
      },
      adapters: [],
      official: [],
    });
    getDataDirs.mockResolvedValue({ home: "/data", logs: "/logs", backups: "/backups" });
    listBackups.mockResolvedValue({
      backups: [
        {
          id: "manual-20260819",
          path: "/backups/manual-20260819.zip",
          createdAt: "2026-08-19T00:00:00Z",
          kind: "manual",
          bytes: 1024,
        },
      ],
    });
    restoreBackup.mockResolvedValue({ imported: 4, skipped: 0, errors: [] });
    inspectZipArchive.mockResolvedValue({ mode: "restore" });
    planClearAllData.mockResolvedValue({
      confirmPhrase: "CLEAR",
      categories: [{ name: "Library", path: "/data/library" }],
    });
    clearAllData.mockResolvedValue(undefined);
    cancelOfficialAction.mockResolvedValue({ ok: true, cancelled: true });
    updaterState = {
      update: null,
      checking: false,
      installing: false,
      progress: null,
      error: null,
      check: checkUpdate,
      install: installUpdate,
    };
    installUpdate.mockResolvedValue({
      ok: true,
      restartRequired: true,
      error: null,
      releasePage: "https://github.com/Jia-Ethan/keysmith-switch-releases/releases",
      installMode: "inApp",
      reason: null,
    });
  });

  it("requires confirmation before replacing data from a backup", async () => {
    const onDataChanged = vi.fn();
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        onDataChanged={onDataChanged}
        toast={toast}
        initialTab="data"
      />,
    );

    await screen.findByText("manual-20260819");
    fireEvent.click(screen.getByRole("button", { name: "恢复" }));

    expect(restoreBackup).not.toHaveBeenCalled();
    expect(screen.getByText(/提示词库、版本历史、激活记录和设置可能被替换/)).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("restore-backup-confirm"));
    await waitFor(() =>
      expect(restoreBackup).toHaveBeenCalledWith("/backups/manual-20260819.zip"),
    );
    expect(onDataChanged).toHaveBeenCalledTimes(1);
    expect(toast.ok).toHaveBeenCalledWith("归档已应用（4 个提示词）");
  });

  it("reports backup creation failures instead of leaving an unhandled rejection", async () => {
    const error = new Error("disk full");
    createBackup.mockRejectedValue(error);
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="data"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "立即备份" }));
    await waitFor(() => expect(toast.err).toHaveBeenCalledWith(error));
  });

  it("shows a retryable error instead of treating backup load failure as empty", async () => {
    listBackups.mockRejectedValue(new Error("database unavailable"));
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="data"
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("无法加载备份");
    expect(screen.queryByTestId("data-backups-empty")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    expect(listBackups).toHaveBeenCalledTimes(2);
  });

  it("locks destructive confirmation while clear-all is running", async () => {
    let finishClear: (() => void) | undefined;
    clearAllData.mockImplementation(
      () => new Promise<void>((resolve) => { finishClear = resolve; }),
    );
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="data"
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "清除全部数据" }));
    await screen.findByPlaceholderText("CLEAR");
    fireEvent.change(screen.getByPlaceholderText("CLEAR"), { target: { value: "CLEAR" } });
    fireEvent.click(screen.getByRole("checkbox", { name: /不可恢复/ }));
    fireEvent.click(screen.getByTestId("clear-all-confirm"));

    await waitFor(() => expect(clearAllData).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("dialog")).toHaveAttribute("aria-busy", "true");
    expect(screen.getByTestId("clear-all-confirm")).toBeDisabled();
    expect(screen.getByRole("button", { name: "取消" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "关闭" })).toBeDisabled();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    finishClear?.();
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("supports standard keyboard navigation between settings tabs", async () => {
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
      />,
    );

    const general = screen.getByRole("tab", { name: "通用" });
    general.focus();
    fireEvent.keyDown(general, { key: "ArrowRight" });
    await waitFor(() => expect(screen.getByRole("tab", { name: "工具" })).toHaveFocus());
    expect(screen.getByRole("tab", { name: "工具" })).toHaveAttribute("aria-selected", "true");

    fireEvent.keyDown(screen.getByRole("tab", { name: "工具" }), { key: "Home" });
    await waitFor(() => expect(screen.getByRole("tab", { name: "通用" })).toHaveFocus());
  });

  it("keeps a manual update check available without release-signing copy", async () => {
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="about"
      />,
    );

    fireEvent.click(screen.getByTestId("check-update"));
    expect(checkUpdate).toHaveBeenCalledTimes(1);
    const section = screen.getByTestId("update-section");
    expect(section).not.toHaveTextContent(/Preview|未签名|Developer ID|Authenticode|公证/i);
  });

  it("requires dialog confirmation before updating and restarting", async () => {
    updaterState.update = {
      available: true,
      currentVersion: "0.1.1",
      latestVersion: "0.1.2",
      notes: "Release notes",
      size: 1_048_576,
      channel: "stable",
      restartRequired: true,
      progress: null,
      error: null,
      releasePage: "https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.1.2",
      installMode: "inApp",
      reason: null,
    };

    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="about"
      />,
    );

    expect(screen.getByText("0.1.1 → 0.1.2")).toBeInTheDocument();
    expect(screen.getByText(/发现新版本 · 1\.0 MB/)).toBeInTheDocument();
    const install = screen.getByTestId("install-update");
    expect(install).not.toBeDisabled();
    fireEvent.click(install);
    expect(installUpdate).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "更新并重启" })).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("confirm-install-update"));
    await waitFor(() => expect(installUpdate).toHaveBeenCalledTimes(1));
  });

  it("shows only the release download action for a bootstrap-required manual update", async () => {
    const releasePage = "https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.1.3";
    updaterState.update = {
      available: true,
      currentVersion: "0.1.1",
      latestVersion: "0.1.3",
      notes: null,
      size: 0,
      channel: "stable",
      restartRequired: false,
      progress: null,
      error: null,
      releasePage,
      installMode: "manual",
      reason: "bootstrapRequired",
    };

    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="about"
      />,
    );

    expect(screen.getByTestId("manual-update-message")).toHaveTextContent(
      "当前版本需要先手动升级，之后即可继续使用应用内更新。",
    );
    expect(screen.queryByText("0 B")).not.toBeInTheDocument();
    expect(screen.queryByTestId("install-update")).not.toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "更新并重启" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("open-update-release"));
    expect(openExternal).toHaveBeenCalledWith(releasePage);
    expect(installUpdate).not.toHaveBeenCalled();
  });

  it("localizes a signing-key mismatch without exposing the backend error", () => {
    updaterState.error = "UnexpectedKeyId: public key 1234 does not match";
    updaterState.update = {
      available: true,
      currentVersion: "0.1.3",
      latestVersion: "0.1.4",
      notes: null,
      size: null,
      channel: "stable",
      restartRequired: false,
      progress: null,
      error: null,
      releasePage: "https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.1.4",
      installMode: "manual",
      reason: "signatureKeyMismatch",
    };

    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="about"
      />,
    );

    expect(screen.getByTestId("manual-update-message")).toHaveTextContent(
      "此更新使用了新的发布签名。为确保安全，请从官方下载页手动安装。",
    );
    expect(screen.queryByText(/UnexpectedKeyId|public key 1234/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not display an unknown update size", () => {
    updaterState.update = {
      available: true,
      currentVersion: "0.1.3",
      latestVersion: "0.1.4",
      notes: null,
      size: null,
      channel: "stable",
      restartRequired: true,
      progress: null,
      error: null,
      releasePage: "https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.1.4",
      installMode: "inApp",
      reason: null,
    };

    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="about"
      />,
    );

    expect(screen.getByText("发现新版本")).toBeInTheDocument();
    expect(screen.queryByText("0 B")).not.toBeInTheDocument();
  });

  it("shows explicit empty states for adapters and official products", async () => {
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="tools"
      />,
    );

    expect(await screen.findByTestId("settings-adapters-empty")).toHaveTextContent("无");
    expect(screen.getByTestId("settings-official-empty")).toHaveTextContent("无");
  });

  it("locks repeated official cancellation and reports when nothing was running", async () => {
    let resolveRun: ((value: { ok: boolean; product: "claude"; action: "update"; error: string | null }) => void) | undefined;
    let resolveCancel: ((value: { ok: boolean; cancelled: boolean }) => void) | undefined;
    confirmOfficialAction.mockImplementation(
      () => new Promise((resolve) => { resolveRun = resolve; }),
    );
    cancelOfficialAction.mockImplementation(
      () => new Promise((resolve) => { resolveCancel = resolve; }),
    );
    getAbout.mockResolvedValue({
      app: {
        name: "Keysmith Switch",
        version: "0.1.1",
        channel: "stable",
        preview: true,
        signed: false,
        identifier: "com.jia-ethan.keysmith-switch",
        website: "https://github.com/Jia-Ethan/keysmith-switch",
        github: "https://github.com/Jia-Ethan/keysmith-switch",
      },
      adapters: [],
      official: [{
        product: "claude",
        currentVersion: "2.1.212",
        latestVersion: "2.1.238",
        installed: true,
        executablePath: "/usr/local/bin/claude",
        source: "npm",
        argv: ["npm", "install", "-g", "@anthropic-ai/claude-code"],
        dest: "/usr/local/bin/claude",
        available: true,
        unavailableReason: null,
      }],
    });
    planOfficialAction.mockResolvedValue({
      planId: "official-1",
      product: "claude",
      action: "update",
      currentVersion: "2.1.212",
      latestVersion: "2.1.238",
      installed: true,
      executablePath: "/usr/local/bin/claude",
      source: "npm",
      argv: ["npm", "install", "-g", "@anthropic-ai/claude-code"],
      dest: "/usr/local/bin/claude",
      blockers: [],
    });

    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="tools"
      />,
    );

    fireEvent.click(await screen.findByTestId("official-plan-claude"));
    await screen.findByTestId("official-plan");
    fireEvent.click(screen.getByTestId("confirm-official"));
    fireEvent.click(screen.getByTestId("run-official"));
    await waitFor(() => expect(confirmOfficialAction).toHaveBeenCalledTimes(1));

    const cancel = screen.getByTestId("cancel-official");
    fireEvent.click(cancel);
    fireEvent.click(cancel);
    expect(cancelOfficialAction).toHaveBeenCalledTimes(1);
    expect(cancel).toBeDisabled();

    await act(async () => {
      resolveCancel?.({ ok: true, cancelled: false });
    });
    expect(toast.info).toHaveBeenCalledWith("当前没有正在执行的官方 CLI 操作。");
    await waitFor(() => expect(screen.getByTestId("cancel-official")).not.toBeDisabled());

    await act(async () => {
      resolveRun?.({ ok: false, product: "claude", action: "update", error: "cancelled by user" });
    });
  });

  it("keeps official CLI execution behind preview and explicit confirmation", async () => {
    getAbout.mockResolvedValue({
      app: {
        name: "Keysmith Switch",
        version: "0.1.1",
        channel: "stable",
        preview: true,
        signed: false,
        identifier: "com.jia-ethan.keysmith-switch",
        website: "https://github.com/Jia-Ethan/keysmith-switch",
        github: "https://github.com/Jia-Ethan/keysmith-switch",
      },
      adapters: [{ tool: "claude", version: "7.1", bundled: true, path: "/app/keysmith-claude" }],
      official: [{
        product: "claude",
        currentVersion: "2.1.212",
        latestVersion: "2.1.238",
        installed: true,
        executablePath: "/usr/local/bin/claude",
        source: "npm",
        argv: ["npm", "install", "-g", "@anthropic-ai/claude-code"],
        dest: "/usr/local/bin/claude",
        available: true,
        unavailableReason: null,
      }],
    });
    planOfficialAction.mockResolvedValue({
      planId: "official-1",
      product: "claude",
      action: "update",
      currentVersion: "2.1.212",
      latestVersion: "2.1.238",
      installed: true,
      executablePath: "/usr/local/bin/claude",
      source: "npm",
      argv: ["npm", "install", "-g", "@anthropic-ai/claude-code"],
      dest: "/usr/local/bin/claude",
      blockers: [],
    });
    confirmOfficialAction.mockResolvedValue({ ok: true, product: "claude", action: "update", error: null });

    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="tools"
      />,
    );

    fireEvent.click(await screen.findByTestId("official-plan-claude"));
    expect(await screen.findByTestId("official-plan")).toBeInTheDocument();
    expect(confirmOfficialAction).not.toHaveBeenCalled();
    expect(screen.getByTestId("run-official")).toBeDisabled();
    fireEvent.click(screen.getByTestId("confirm-official"));
    fireEvent.click(screen.getByTestId("run-official"));
    await waitFor(() => expect(confirmOfficialAction).toHaveBeenCalledWith("official-1"));
  });
});
