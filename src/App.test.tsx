import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./api", () => ({
  getSettings: vi.fn().mockRejectedValue(new Error("no backend")),
  updateSettings: vi.fn(),
  listTools: vi.fn().mockRejectedValue(new Error("no backend")),
  listPrompts: vi.fn().mockRejectedValue(new Error("no backend")),
  toolStatus: vi.fn().mockRejectedValue(new Error("no backend")),
  doctor: vi.fn().mockRejectedValue(new Error("no backend")),
  listOperations: vi.fn().mockRejectedValue(new Error("no backend")),
  listActivations: vi.fn().mockRejectedValue(new Error("no backend")),
  getAbout: vi.fn().mockRejectedValue(new Error("no backend")),
  checkAppUpdate: vi.fn().mockRejectedValue(new Error("no backend")),
  getStartupReport: vi.fn().mockResolvedValue({ firstRun: false, candidates: [], recovery: null, sidecar: null }),
  getHarnessState: vi.fn().mockResolvedValue({ tool: "claude", deployed: false, error: null }),
  logFrontendError: vi.fn(),
  showMainWindow: vi.fn(),
  quitApp: vi.fn(),
}));

describe("App smoke", () => {
  it("mounts on the harness page with the single undeployed action", async () => {
    const { App } = await import("./App");
    render(<App />);
    expect(screen.getByTestId("nav-claude")).toBeInTheDocument();
    expect(screen.getByTestId("nav-settings")).toBeInTheDocument();
    expect(await screen.findByTestId("harness-deploy")).toBeInTheDocument();
    expect(screen.queryByTestId("harness-remove")).not.toBeInTheDocument();
    expect(screen.queryByTestId("prompt-search")).not.toBeInTheDocument();
    expect(screen.queryByTestId("prompt-new")).not.toBeInTheDocument();
    expect(screen.queryByTestId("scope-bar")).not.toBeInTheDocument();
  });
});
