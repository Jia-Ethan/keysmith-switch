import { fireEvent, render, screen } from "@testing-library/react";
import { waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AppShell, type AppPage } from "./AppShell";
import { UpdateProvider } from "./UpdateProvider";

const checkAppUpdate = vi.fn();
vi.mock("../api", () => ({
  checkAppUpdate: (...args: unknown[]) => checkAppUpdate(...args),
  getHarnessState: vi.fn().mockResolvedValue({ tool: "claude", deployed: false, error: null }),
}));

const releaseInfo = {
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

function renderShell(page: AppPage, advancedEnabled = false, autoCheck = false) {
  const onNavigate = vi.fn();
  const view = render(
    <UpdateProvider channel="stable" autoCheck={autoCheck} minCheckMs={0}>
      <AppShell page={page} onNavigate={onNavigate} advancedEnabled={advancedEnabled}>
        <div>content</div>
      </AppShell>
    </UpdateProvider>,
  );
  return { onNavigate, unmount: view.unmount };
}

describe("AppShell tool navigation", () => {
  it("renders all four tool entries", () => {
    renderShell({ kind: "tool", tool: "claude" });
    for (const tool of ["claude", "codex", "grok", "zcode"]) {
      expect(screen.getByTestId(`nav-${tool}`)).toBeInTheDocument();
    }
  });

  it("marks only the current tool as the active page", () => {
    renderShell({ kind: "tool", tool: "codex" });
    expect(screen.getByTestId("nav-codex")).toHaveAttribute("aria-current", "page");
    expect(screen.getByTestId("nav-claude")).not.toHaveAttribute("aria-current");
    expect(screen.getByTestId("nav-grok")).not.toHaveAttribute("aria-current");
    expect(screen.getByTestId("nav-zcode")).not.toHaveAttribute("aria-current");
  });

  it("routes tool clicks through onNavigate", () => {
    const { onNavigate } = renderShell({ kind: "tool", tool: "claude" });
    fireEvent.click(screen.getByTestId("nav-zcode"));
    expect(onNavigate).toHaveBeenCalledWith({ kind: "tool", tool: "zcode" });
  });

  it("shows a gear settings control and hides Advanced Tools by default", () => {
    renderShell({ kind: "tool", tool: "claude" });
    const settings = screen.getByTestId("nav-settings");
    expect(settings).toBeInTheDocument();
    expect(settings.querySelector("circle")).toBeInTheDocument();
    // A closed gear outline, not the old ray burst of separate strokes.
    expect(settings.querySelector("path")?.getAttribute("d")).toMatch(/Z$/);
    expect(settings.querySelector("path")?.getAttribute("d")).not.toContain("M8 1.7v1.55");
    expect(screen.queryByTestId("nav-advanced")).not.toBeInTheDocument();
  });

  it("shows Advanced Tools only once enabled in settings", () => {
    renderShell({ kind: "tool", tool: "claude" }, true);
    expect(screen.getByTestId("nav-advanced")).toBeInTheDocument();
  });

  it("does not mark any tool active on the settings page", () => {
    renderShell({ kind: "settings" });
    for (const tool of ["claude", "codex", "grok", "zcode"]) {
      expect(screen.getByTestId(`nav-${tool}`)).not.toHaveAttribute("aria-current");
    }
    expect(screen.getByTestId("nav-settings")).toBeInTheDocument();
  });

  it("keeps every entry on the rail without an overflow menu", () => {
    renderShell({ kind: "tool", tool: "claude" }, true);
    expect(screen.queryByTestId("nav-more")).not.toBeInTheDocument();
    expect(screen.getByRole("navigation")).toContainElement(screen.getByTestId("nav-zcode"));
    expect(screen.getByRole("navigation")).toContainElement(screen.getByTestId("nav-settings"));
  });

  it("marks settings and advanced as the current page on their own pages", () => {
    renderShell({ kind: "settings" }, true);
    expect(screen.getByTestId("nav-settings")).toHaveAttribute("aria-current", "page");
    expect(screen.getByTestId("nav-advanced")).not.toHaveAttribute("aria-current");
  });

  it("opens the About page from the app icon", () => {
    const { onNavigate } = renderShell({ kind: "tool", tool: "claude" });
    fireEvent.click(screen.getByTestId("nav-brand"));
    expect(onNavigate).toHaveBeenCalledWith({ kind: "settings", tab: "about" });
  });

  it("shows a dot on settings when a newer version is out, and none otherwise", async () => {
    checkAppUpdate.mockResolvedValue({ ...releaseInfo, available: false });
    const first = renderShell({ kind: "tool", tool: "claude" }, false, true);
    await waitFor(() => expect(checkAppUpdate).toHaveBeenCalled(), { timeout: 4000 });
    expect(screen.queryByTestId("notice-dot")).not.toBeInTheDocument();
    first.unmount();

    checkAppUpdate.mockResolvedValue({ ...releaseInfo, available: true });
    renderShell({ kind: "tool", tool: "claude" }, false, true);
    const dot = await screen.findByTestId("notice-dot", undefined, { timeout: 4000 });
    expect(screen.getByTestId("nav-settings")).toContainElement(dot);
    expect(screen.getByTestId("nav-settings")).toHaveAttribute("aria-label", "设置 · 有新版本");
  });
});
