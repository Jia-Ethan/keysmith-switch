import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AppShell, countVisibleNavItems, type AppPage } from "./AppShell";
import { UpdateProvider } from "./UpdateProvider";

function renderShell(page: AppPage, advancedEnabled = false) {
  const onNavigate = vi.fn();
  render(
    <UpdateProvider channel="stable" autoCheck={false}>
      <AppShell page={page} onNavigate={onNavigate} advancedEnabled={advancedEnabled}>
        <div>content</div>
      </AppShell>
    </UpdateProvider>,
  );
  return { onNavigate };
}

class TestResizeObserver {
  static instances: TestResizeObserver[] = [];

  readonly observe = vi.fn();
  readonly disconnect = vi.fn();

  constructor(private readonly callback: ResizeObserverCallback) {
    TestResizeObserver.instances.push(this);
  }

  trigger() {
    this.callback([], this as unknown as ResizeObserver);
  }
}

afterEach(() => {
  TestResizeObserver.instances = [];
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

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

  it("shows settings and hides Advanced Tools by default", () => {
    renderShell({ kind: "tool", tool: "claude" });
    expect(screen.getByTestId("nav-settings")).toBeInTheDocument();
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

  it("opens overflow navigation in a narrow slot and routes its items", () => {
    vi.stubGlobal("ResizeObserver", TestResizeObserver);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (
      this: HTMLElement,
    ) {
      const id = this.getAttribute("data-nav-measure");
      return id === "more" ? 48 : id ? 88 : 0;
    });
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(
      function (this: HTMLElement) {
        return this.querySelector('[data-nav-measure="more"]') ? 188 : 0;
      },
    );

    const { onNavigate } = renderShell({ kind: "tool", tool: "claude" });
    TestResizeObserver.instances[0]?.trigger();

    fireEvent.click(screen.getByTestId("nav-more"));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("nav-overflow-zcode"));
    expect(onNavigate).toHaveBeenCalledWith({ kind: "tool", tool: "zcode" });
  });
});

describe("countVisibleNavItems", () => {
  const iconOnly = [44, 44, 44, 44];
  const labeled = [108, 104, 92, 112];

  it("keeps all four tools in a 1440-class slot", () => {
    expect(countVisibleNavItems(720, labeled, 48)).toBe(4);
    expect(countVisibleNavItems(520, iconOnly, 48)).toBe(4);
  });

  it("keeps all four icon-only tools in a 1180-class slot", () => {
    expect(countVisibleNavItems(360, iconOnly, 48)).toBe(4);
  });

  it("reserves overflow around 760-class and 520-class slots", () => {
    expect(countVisibleNavItems(200, labeled, 48)).toBeLessThan(4);
    expect(countVisibleNavItems(140, iconOnly, 48)).toBeGreaterThanOrEqual(1);
    expect(countVisibleNavItems(140, iconOnly, 48)).toBeLessThan(4);
  });

  it("keeps at least one tool plus overflow in a 360-class slot", () => {
    expect(countVisibleNavItems(96, iconOnly, 48)).toBe(1);
    expect(countVisibleNavItems(80, labeled, 48)).toBe(1);
  });
});
