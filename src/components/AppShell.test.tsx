import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AppShell, type AppPage } from "./AppShell";
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
});
