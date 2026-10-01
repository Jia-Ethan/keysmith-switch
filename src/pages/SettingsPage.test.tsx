import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n";
import type { ToastApi } from "../hooks/useToasts";
import { DEFAULT_SETTINGS, type UpdateCheck } from "../types";
import { SettingsPage } from "./SettingsPage";

const getAbout = vi.fn();
const checkUpdate = vi.fn();
const installUpdate = vi.fn();
const openExternal = vi.fn();
let updaterState: {
  update: UpdateCheck | null;
  hasUpdate: boolean;
  checking: boolean;
  installing: boolean;
  progress: number | null;
  error: string | null;
  checkCount: number;
  check: typeof checkUpdate;
  install: typeof installUpdate;
};

vi.mock("../api", () => ({
  getAbout: (...args: unknown[]) => getAbout(...args),
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
    updaterState = {
      update: null,
      hasUpdate: false,
      checking: false,
      installing: false,
      progress: null,
      error: null,
      checkCount: 0,
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

  it("does not offer a data and backup page", () => {
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="data"
      />,
    );

    expect(screen.queryByRole("tab", { name: "数据与备份" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "立即备份" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "清除全部数据" })).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "通用" })).toHaveAttribute("aria-selected", "true");
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

  it("shows a failed update check with a download fallback, not an up-to-date result", () => {
    updaterState.update = {
      available: false,
      currentVersion: "0.1.4",
      latestVersion: null,
      notes: null,
      size: null,
      channel: "stable",
      restartRequired: false,
      progress: null,
      error: "offline: network unavailable",
      releasePage: "https://github.com/Jia-Ethan/keysmith-switch-releases/releases",
      installMode: "none",
      reason: null,
    };
    updaterState.error = updaterState.update.error;
    updaterState.checkCount = 2;
    render(<SettingsPage settings={DEFAULT_SETTINGS} onSave={vi.fn()} toast={toast} initialTab="about" />);

    const section = screen.getByTestId("update-section");
    expect(section).toHaveTextContent("offline: network unavailable");
    expect(section).not.toHaveTextContent("已是最新版本");
    fireEvent.click(screen.getByTestId("open-update-release-on-error"));
    expect(openExternal).toHaveBeenCalledWith(updaterState.update.releasePage);
  });

  it("exposes the official download after an in-app installation failure", () => {
    const releasePage = "https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.1.5";
    updaterState.update = {
      available: true,
      currentVersion: "0.1.4",
      latestVersion: "0.1.5",
      notes: null,
      size: 1_048_576,
      channel: "stable",
      restartRequired: true,
      progress: null,
      error: null,
      releasePage,
      installMode: "inApp",
      reason: null,
    };
    updaterState.error = "signature verification failed";
    render(<SettingsPage settings={DEFAULT_SETTINGS} onSave={vi.fn()} toast={toast} initialTab="about" />);

    expect(screen.getByTestId("update-section")).toHaveTextContent("signature verification failed");
    expect(screen.queryByTestId("install-update")).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId("open-update-release-on-error"));
    expect(openExternal).toHaveBeenCalledWith(releasePage);
  });

  it("keeps the checking label visible until the pending request finishes", () => {
    updaterState.checking = true;
    render(<SettingsPage settings={DEFAULT_SETTINGS} onSave={vi.fn()} toast={toast} initialTab="about" />);
    expect(screen.getByTestId("check-update")).toHaveTextContent("正在检查更新");
    expect(screen.getByTestId("check-update")).toBeDisabled();
  });

  it("shows progress on the button and in the result area while a check runs", () => {
    updaterState.checking = true;
    render(<SettingsPage settings={DEFAULT_SETTINGS} onSave={vi.fn()} toast={toast} initialTab="about" />);
    expect(screen.getByTestId("check-update")).toHaveAttribute("aria-busy", "true");
    expect(screen.getByTestId("check-update").querySelector(".spinner")).toBeInTheDocument();
    expect(screen.getByTestId("update-checking")).toHaveTextContent("正在检查更新");
  });

  it("does not show the checking line once the check has finished", () => {
    render(<SettingsPage settings={DEFAULT_SETTINGS} onSave={vi.fn()} toast={toast} initialTab="about" />);
    expect(screen.getByTestId("check-update")).not.toHaveAttribute("aria-busy");
    expect(screen.queryByTestId("update-checking")).not.toBeInTheDocument();
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
    expect(screen.getByText(/发现新版本 · 0\.1\.2 · 1\.0 MB/)).toBeInTheDocument();
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
      "v0.1.1 内置的是测试更新公钥，无法安装带正式签名的版本。请从官方下载页手动安装，当前版本会保持不变。",
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
      "更新包的签名与本机内置公钥不一致，无法在应用内安装。请从官方下载页手动安装，当前版本会保持不变。",
    );
    expect(screen.queryByText(/UnexpectedKeyId|public key 1234/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByTestId("install-update")).not.toBeInTheDocument();
  });

  it("keeps the raw signature error inside details and still links the release page", () => {
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
      reason: "signatureKeyMismatch",
      detail: {
        code: "signature_key_mismatch",
        message: "The signature was created with a different key than the one provided",
      },
    };

    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="about"
      />,
    );

    expect(screen.getByTestId("manual-update-message")).toHaveTextContent("手动安装");
    expect(screen.queryByText("The signature was created with a different key than the one provided")).not.toBeVisible();
    fireEvent.click(screen.getByText("详情"));
    expect(screen.getByTestId("update-error-details")).toHaveTextContent(
      "The signature was created with a different key than the one provided",
    );
    expect(screen.queryByText("0 B")).not.toBeInTheDocument();
    expect(screen.queryByTestId("install-update")).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId("open-update-release"));
    expect(openExternal).toHaveBeenCalledWith(releasePage);
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

    expect(screen.getByText(/发现新版本/)).toBeInTheDocument();
    expect(screen.queryByText("0 B")).not.toBeInTheDocument();
  });

  it("lists the four Keysmith repositories and nothing else", async () => {
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="tools"
      />,
    );

    expect(await screen.findByTestId("keysmith-claude")).toHaveTextContent("Claude Keysmith");
    expect(screen.getByTestId("keysmith-codex")).toHaveTextContent("Codex Keysmith");
    expect(screen.getByTestId("keysmith-grok")).toHaveTextContent("Grok Keysmith");
    expect(screen.getByTestId("keysmith-zcode")).toHaveTextContent("Zcode Keysmith");
    fireEvent.click(screen.getByTestId("keysmith-repo-grok"));
    expect(openExternal).toHaveBeenCalledWith("https://github.com/Jia-Ethan/grok-keysmith");
    expect(screen.queryByText("官方产品")).not.toBeInTheDocument();
    expect(screen.queryByText("高级工具")).not.toBeInTheDocument();
    expect(screen.queryByText(/npm is required|\/app\//)).not.toBeInTheDocument();
  });

  it("keeps general to language and theme, with no tray or launch settings", () => {
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        onSave={vi.fn()}
        toast={toast}
        initialTab="general"
      />,
    );

    expect(screen.getByLabelText("界面语言")).toBeInTheDocument();
    expect(screen.getByLabelText("主题")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "浅色" }).querySelector("circle")).toBeInTheDocument();
    expect(screen.getByTestId("theme-sun")).toBeInTheDocument();
    expect(screen.queryByLabelText("关闭窗口进入托盘")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("开机启动")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("静默启动")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Claude Code 默认范围")).not.toBeInTheDocument();
    expect(screen.queryByTestId("settings-nav-data")).not.toBeInTheDocument();
    expect(screen.queryByTestId("settings-shortcuts")).not.toBeInTheDocument();
    expect(screen.queryByText("快捷键")).not.toBeInTheDocument();
  });

  it("acknowledges a saved setting quietly beside the title, not with a toast", async () => {
    const onSave = vi.fn().mockResolvedValue(DEFAULT_SETTINGS);
    render(
      <SettingsPage settings={DEFAULT_SETTINGS} onSave={onSave} toast={toast} initialTab="general" />,
    );

    expect(screen.queryByTestId("settings-saved")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("radio", { name: "深色" }));
    expect(await screen.findByTestId("settings-saved")).toHaveTextContent("设置已保存");
    expect(toast.ok).not.toHaveBeenCalled();
  });

  it("still reports a failed save as an error toast", async () => {
    const onSave = vi.fn().mockRejectedValue(new Error("disk full"));
    render(
      <SettingsPage settings={DEFAULT_SETTINGS} onSave={onSave} toast={toast} initialTab="general" />,
    );

    fireEvent.click(screen.getByRole("radio", { name: "深色" }));
    await waitFor(() => expect(toast.err).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId("settings-saved")).not.toBeInTheDocument();
  });

  it("opens the language list inside the page, not as a system overlay", async () => {
    const onSave = vi.fn().mockResolvedValue(DEFAULT_SETTINGS);
    render(
      <SettingsPage settings={DEFAULT_SETTINGS} onSave={onSave} toast={toast} initialTab="general" />,
    );

    // The list does not exist until the trigger is used, and it is a listbox in
    // the page rather than a native select the platform would paint itself.
    expect(screen.queryByTestId("settings-language-menu")).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId("settings-language"));
    expect(screen.getByTestId("settings-language-menu")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "简体中文" })).toHaveAttribute("aria-selected", "true");
    // Rendered outside the settings card, whose clipped overflow would cut it off.
    expect(screen.getByTestId("settings-language-menu").parentElement).toBe(document.body);

    fireEvent.click(screen.getByTestId("settings-language-en"));
    await waitFor(() => expect(onSave).toHaveBeenCalledWith({ language: "en" }));
    expect(screen.queryByTestId("settings-language-menu")).not.toBeInTheDocument();
  });

  it("closes the language list on Escape without saving", () => {
    const onSave = vi.fn().mockResolvedValue(DEFAULT_SETTINGS);
    render(
      <SettingsPage settings={DEFAULT_SETTINGS} onSave={onSave} toast={toast} initialTab="general" />,
    );

    fireEvent.click(screen.getByTestId("settings-language"));
    fireEvent.keyDown(screen.getByTestId("settings-language"), { key: "Escape" });
    expect(screen.queryByTestId("settings-language-menu")).not.toBeInTheDocument();
    expect(onSave).not.toHaveBeenCalled();
  });

  const availableInfo: UpdateCheck = {
    available: true,
    currentVersion: "0.2.3",
    latestVersion: "0.2.4",
    notes: null,
    size: null,
    channel: "stable",
    restartRequired: true,
    progress: null,
    error: null,
    releasePage: "https://example.test/releases",
    installMode: "inApp",
    reason: null,
  };

  it("marks the About tab with a dot while a newer version is waiting", () => {
    updaterState.update = availableInfo;
    updaterState.hasUpdate = true;
    render(<SettingsPage settings={DEFAULT_SETTINGS} onSave={vi.fn()} toast={toast} />);
    expect(screen.getByTestId("settings-nav-about")).toContainElement(screen.getByTestId("notice-dot"));
    expect(screen.getByTestId("settings-nav-general")).not.toContainElement(screen.getByTestId("notice-dot"));
  });

  it("shows no dot on the About tab when the app is current", () => {
    updaterState.update = { ...availableInfo, available: false, latestVersion: null };
    render(<SettingsPage settings={DEFAULT_SETTINGS} onSave={vi.fn()} toast={toast} initialTab="about" />);
    expect(screen.queryByTestId("notice-dot")).not.toBeInTheDocument();
    expect(screen.getByTestId("update-section")).toHaveAttribute("data-state", "upToDate");
  });

  it("keeps the orb on its icon while a new check runs, so a quick answer does not flash", () => {
    updaterState.update = { ...availableInfo, available: false, latestVersion: null };
    const ui = () => <SettingsPage settings={DEFAULT_SETTINGS} onSave={vi.fn()} toast={toast} initialTab="about" />;
    const view = render(ui());
    const orb = () => screen.getByTestId("update-section").querySelector(".update-orb")!;
    expect(orb()).toHaveAttribute("data-state", "upToDate");
    expect(orb()).not.toHaveAttribute("data-busy");

    updaterState.update = null;
    updaterState.checking = true;
    view.rerender(ui());
    expect(orb()).toHaveAttribute("data-state", "upToDate");
    expect(orb()).toHaveAttribute("data-busy");
  });

  it("keeps the last answer on screen, dimmed, while a new check runs", () => {
    updaterState.update = availableInfo;
    const ui = () => <SettingsPage settings={DEFAULT_SETTINGS} onSave={vi.fn()} toast={toast} initialTab="about" />;
    const view = render(ui());
    expect(screen.getByTestId("update-section")).toHaveAttribute("data-state", "available");

    // A new check clears the answer in the provider; the page must not collapse and reopen,
    // and the dot must not blink out (the provider keeps knowing a release is out).
    updaterState.update = null;
    updaterState.hasUpdate = true;
    updaterState.checking = true;
    view.rerender(ui());
    expect(screen.getByTestId("settings-nav-about")).toContainElement(screen.getByTestId("notice-dot"));
    expect(screen.getByTestId("update-section")).toHaveAttribute("data-state", "checking");
    expect(screen.getByTestId("install-update")).toBeInTheDocument();
    expect(screen.getByTestId("install-update").closest(".pointer-events-none")).not.toBeNull();
    expect(screen.getByTestId("update-checking")).toHaveTextContent("正在检查更新");
  });
});
