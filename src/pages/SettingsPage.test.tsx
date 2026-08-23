import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n";
import type { ToastApi } from "../hooks/useToasts";
import { DEFAULT_SETTINGS } from "../types";
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
  openExternal: vi.fn(),
  pickFiles: vi.fn().mockResolvedValue([]),
  pickSavePath: vi.fn().mockResolvedValue(null),
  isTauriRuntime: vi.fn().mockReturnValue(false),
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
