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
  getStartupReport: vi.fn().mockRejectedValue(new Error("no backend")),
  logFrontendError: vi.fn(),
}));

describe("App smoke", () => {
  it("mounts and lands on the tool page without crashing", async () => {
    const { App } = await import("./App");
    render(<App />);
    expect(screen.getByTestId("nav-claude")).toBeInTheDocument();
    expect(screen.getByTestId("nav-settings")).toBeInTheDocument();
    // no marketing hero, straight into tool management
    expect(screen.getByTestId("prompt-search")).toBeInTheDocument();
  });
});
