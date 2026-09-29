import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./api", () => ({
  getSettings: vi.fn().mockRejectedValue(new Error("no backend")),
  updateSettings: vi.fn(),
  listTools: vi.fn().mockResolvedValue({ tools: [] }),
  listPrompts: vi.fn().mockResolvedValue({ prompts: [] }),
  doctor: vi.fn().mockRejectedValue(new Error("no backend")),
  listOperations: vi.fn().mockRejectedValue(new Error("no backend")),
  listActivations: vi.fn().mockRejectedValue(new Error("no backend")),
  getAbout: vi.fn().mockRejectedValue(new Error("no backend")),
  checkAppUpdate: vi.fn().mockRejectedValue(new Error("no backend")),
  getStartupReport: vi.fn().mockResolvedValue({ firstRun: false, candidates: [], recovery: null, sidecar: null }),
  getHarnessState: vi.fn().mockResolvedValue({ tool: "claude", deployed: false, error: null }),
  toolStatus: vi.fn().mockResolvedValue({
    tool: "claude",
    available: true,
    scopes: [{ id: "user", supported: true, reason: null }],
  }),
  createPastedPrompt: vi.fn(),
  planActivate: vi.fn(),
  activate: vi.fn(),
  planDeactivate: vi.fn(),
  deactivate: vi.fn(),
  logFrontendError: vi.fn(),
  showMainWindow: vi.fn(),
  quitApp: vi.fn(),
}));

describe("App smoke", () => {
  it("mounts on the agent workspace with the library and a Quick Deploy entry", async () => {
    const { App } = await import("./App");
    render(<App />);
    expect(screen.getByTestId("nav-claude")).toBeInTheDocument();
    expect(screen.getByTestId("nav-settings")).toBeInTheDocument();
    expect(await screen.findByTestId("workspace-page")).toBeInTheDocument();
    expect(screen.getByTestId("prompt-search")).toBeInTheDocument();
    expect(screen.queryByTestId("quick-deploy-title")).not.toBeInTheDocument();
    fireEvent.click(await screen.findByTestId("quick-deploy-open"));
    expect(await screen.findByTestId("quick-deploy-title")).toBeInTheDocument();
    expect(document.documentElement.dataset.agent).toBe("claude");
  });

  it("switches agents with mod+number and retints the accent", async () => {
    const { App } = await import("./App");
    render(<App />);
    await screen.findByTestId("workspace-page");
    fireEvent.keyDown(window, { key: "3", ctrlKey: true });
    expect(await screen.findByRole("heading", { name: "Grok Build" })).toBeInTheDocument();
    expect(screen.getByTestId("nav-grok")).toHaveAttribute("aria-current", "page");
    expect(document.documentElement.dataset.agent).toBe("grok");
  });
});
