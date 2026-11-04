import { act, fireEvent, render, screen } from "@testing-library/react";
import { waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppShell, type AppPage } from "./AppShell";
import { ExtensionsProvider } from "./ExtensionsProvider";
import { UpdateProvider } from "./UpdateProvider";

const checkAppUpdate = vi.fn();
const extensionsState = vi.fn();
vi.mock("../api", () => ({
  extensionsState: (...args: unknown[]) => extensionsState(...args),
  extensionsRefresh: vi.fn().mockResolvedValue({ packs: [], updates: 0, error: null, checkedAt: null }),
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
  it("renders all six tool entries", () => {
    renderShell({ kind: "tool", tool: "claude" });
    for (const tool of ["claude", "codex", "grok", "zcode", "pi", "dsh"]) {
      expect(screen.getByTestId(`nav-${tool}`)).toBeInTheDocument();
    }
  });

  it("marks only the current tool as the active page", () => {
    renderShell({ kind: "tool", tool: "codex" });
    expect(screen.getByTestId("nav-codex")).toHaveAttribute("aria-current", "page");
    expect(screen.getByTestId("nav-claude")).not.toHaveAttribute("aria-current");
    expect(screen.getByTestId("nav-grok")).not.toHaveAttribute("aria-current");
    expect(screen.getByTestId("nav-zcode")).not.toHaveAttribute("aria-current");
    expect(screen.getByTestId("nav-pi")).not.toHaveAttribute("aria-current");
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
    for (const tool of ["claude", "codex", "grok", "zcode", "pi", "dsh"]) {
      expect(screen.getByTestId(`nav-${tool}`)).not.toHaveAttribute("aria-current");
    }
    expect(screen.getByTestId("nav-settings")).toBeInTheDocument();
  });

  it("keeps every entry on the rail without an overflow menu", () => {
    renderShell({ kind: "tool", tool: "claude" }, true);
    expect(screen.queryByTestId("nav-more")).not.toBeInTheDocument();
    expect(screen.getByRole("navigation")).toContainElement(screen.getByTestId("nav-pi"));
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

  it("shows the Extensions button only where the provider is, with a count badge for updates", async () => {
    renderShell({ kind: "tool", tool: "claude" });
    expect(screen.queryByTestId("nav-extensions")).not.toBeInTheDocument();

    extensionsState.mockResolvedValue({ packs: [], updates: 2, error: null, checkedAt: null });
    const onNavigate = vi.fn();
    render(
      <ExtensionsProvider enabled onEnabledChange={vi.fn()}>
        <AppShell page={{ kind: "extensions" }} onNavigate={onNavigate} advancedEnabled={false}>
          <div>content</div>
        </AppShell>
      </ExtensionsProvider>,
    );
    const button = await screen.findByTestId("nav-extensions");
    await waitFor(() => {
      const badge = button.querySelector('[data-testid="notice-count"]');
      expect(badge).not.toBeNull();
      expect(badge).toHaveTextContent("2");
    });
    expect(button).toHaveAttribute("aria-label", "拓展 · 2 个可更新");
    expect(button).toHaveAttribute("aria-current", "page");
    fireEvent.click(button);
    expect(onNavigate).toHaveBeenCalledWith({ kind: "extensions" });
  });
});

/**
 * jsdom has no layout, so the rail's heights are given here: the rail (`nav`) has `rail`
 * pixels, and the agent list holds the agents at their named or folded height. Each
 * resize is delivered through a stand-in ResizeObserver, as a real window would.
 */
describe("AppShell rail on a short window", () => {
  const NAMED = 6 * 76 + 5 * 4;
  const FOLDED = 6 * 50 + 5 * 2;
  const UTILITIES = 260;
  let rail = 900;
  const observers = new Set<() => void>();

  const isRail = (el: Element) => el.tagName === "NAV";
  const isAgents = (el: Element) => el.getAttribute("data-testid") === "rail-agents";
  const folded = () => screen.getByTestId("rail-agents").hasAttribute("data-compact");
  const agentsRoom = () => Math.max(0, rail - UTILITIES);

  // index.css gives the scrolling list room for its fades; jsdom does not load that file.
  let fadePadding: HTMLStyleElement;

  beforeEach(() => {
    rail = 900;
    observers.clear();
    fadePadding = document.createElement("style");
    fadePadding.textContent = ".rail-agents-scroll { padding-top: 8px; padding-bottom: 8px; }";
    document.head.appendChild(fadePadding);
    vi.stubGlobal(
      "ResizeObserver",
      class {
        callback: () => void;
        constructor(callback: () => void) {
          this.callback = () => callback();
          observers.add(this.callback);
        }
        observe() {}
        unobserve() {}
        disconnect() {
          observers.delete(this.callback);
        }
      },
    );
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(function (this: HTMLElement) {
      if (isRail(this)) return rail;
      if (isAgents(this)) return agentsRoom();
      return 0;
    });
    vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockImplementation(function (this: HTMLElement) {
      if (!isAgents(this)) return 0;
      const content = this.hasAttribute("data-compact") ? FOLDED : NAMED;
      const padding = this.hasAttribute("data-scrolls") ? 16 : 0;
      return Math.max(content + padding, agentsRoom());
    });
  });

  afterEach(() => {
    fadePadding.remove();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  const resize = (height: number) =>
    act(() => {
      rail = height;
      for (const callback of Array.from(observers)) callback();
    });

  it("shows every agent's name while they fit", () => {
    renderShell({ kind: "tool", tool: "claude" });
    expect(folded()).toBe(false);
    expect(screen.getByTestId("nav-codex")).toHaveTextContent("Codex");
    expect(screen.getByTestId("rail-agents")).not.toHaveAttribute("data-scrolls");
  });

  it("folds the names away when the window is too short, keeping them as tooltips", () => {
    renderShell({ kind: "tool", tool: "claude" });
    resize(640);
    expect(folded()).toBe(true);
    expect(screen.getByTestId("rail-agents")).not.toHaveAttribute("data-scrolls");
    const codex = screen.getByTestId("nav-codex");
    expect(codex).toHaveAttribute("title", "Codex");
    expect(screen.getByRole("button", { name: "Codex" })).toBe(codex);
    expect(codex.querySelector(".sr-only")).toHaveTextContent("Codex");
  });

  it("scrolls only the agents when folding is not enough, and keeps Settings outside the scroll", () => {
    renderShell({ kind: "settings" });
    resize(480);
    const agents = screen.getByTestId("rail-agents");
    expect(folded()).toBe(true);
    expect(agents).toHaveAttribute("data-scrolls", "true");
    expect(agents).not.toContainElement(screen.getByTestId("nav-settings"));
    expect(screen.getByTestId("rail-utilities")).toContainElement(screen.getByTestId("nav-settings"));
  });

  it("brings the names back only once there is room for them again", () => {
    renderShell({ kind: "tool", tool: "claude" });
    resize(640);
    expect(folded()).toBe(true);
    // Room for the folded agents, not yet for the named ones: stay folded rather than flicker.
    resize(UTILITIES + NAMED - 1);
    expect(folded()).toBe(true);
    resize(UTILITIES + NAMED);
    expect(folded()).toBe(false);
  });

  it("stops scrolling as soon as the folded agents fit again", () => {
    renderShell({ kind: "tool", tool: "claude" });
    resize(480);
    expect(screen.getByTestId("rail-agents")).toHaveAttribute("data-scrolls", "true");
    resize(UTILITIES + FOLDED);
    expect(screen.getByTestId("rail-agents")).not.toHaveAttribute("data-scrolls");
    expect(folded()).toBe(true);
  });
});
