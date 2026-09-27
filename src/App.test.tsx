import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./api", () => ({
  getSettings: vi.fn().mockRejectedValue(new Error("no backend")),
  updateSettings: vi.fn(),
  listTools: vi.fn().mockResolvedValue({ tools: [] }),
  listPrompts: vi.fn().mockRejectedValue(new Error("no backend")),
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
  it("mounts on the Quick Deploy page with a pasted prompt form", async () => {
    const { App } = await import("./App");
    render(<App />);
    expect(screen.getByTestId("nav-claude")).toBeInTheDocument();
    expect(screen.getByTestId("nav-settings")).toBeInTheDocument();
    expect(await screen.findByTestId("quick-deploy-submit")).toBeInTheDocument();
    expect(screen.getByTestId("quick-deploy-title")).toBeInTheDocument();
    expect(screen.queryByTestId("harness-deploy")).not.toBeInTheDocument();
    expect(screen.queryByTestId("prompt-search")).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId("quick-deploy-library"));
    expect(await screen.findByTestId("prompt-search")).toBeInTheDocument();
  });
});
