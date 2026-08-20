import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
}));

vi.mock("../lib/runtime", () => ({
  openExternal: vi.fn(),
  pickFiles: vi.fn().mockResolvedValue([]),
  pickSavePath: vi.fn().mockResolvedValue(null),
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
});
