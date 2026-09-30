import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CommandPalette } from "./CommandPalette";
import { DEPLOY_PROMPT_EVENT, QUICK_DEPLOY_EVENT } from "../lib/paletteEvents";

const listPrompts = vi.fn();
vi.mock("../api", () => ({ listPrompts: (...args: unknown[]) => listPrompts(...args) }));

function renderPalette(props: Partial<Parameters<typeof CommandPalette>[0]> = {}) {
  const onClose = vi.fn();
  const onNavigate = vi.fn();
  render(
    <CommandPalette open onClose={onClose} page={{ kind: "tool", tool: "claude" }} onNavigate={onNavigate} {...props} />,
  );
  return { onClose, onNavigate, input: screen.getByTestId("command-palette-input") };
}

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listPrompts.mockResolvedValue({ prompts: [{ id: "p1", title: "Code reviewer", tags: [] }] });
  });

  it("renders nothing while closed", () => {
    render(<CommandPalette open={false} onClose={vi.fn()} page={{ kind: "settings" }} onNavigate={vi.fn()} />);
    expect(screen.queryByTestId("command-palette")).not.toBeInTheDocument();
  });

  it("lists the other agents but not the current one", () => {
    renderPalette();
    expect(screen.getByTestId("palette-item-agent-codex")).toBeInTheDocument();
    expect(screen.queryByTestId("palette-item-agent-claude")).not.toBeInTheDocument();
  });

  it("filters, then switches agent with Enter", () => {
    const { input, onClose, onNavigate } = renderPalette();
    fireEvent.change(input, { target: { value: "zcode" } });
    expect(screen.queryByTestId("palette-item-agent-codex")).not.toBeInTheDocument();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onNavigate).toHaveBeenCalledWith({ kind: "tool", tool: "zcode" });
    expect(onClose).toHaveBeenCalled();
  });

  it("moves with the arrow keys", () => {
    const { input, onNavigate } = renderPalette();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onNavigate).toHaveBeenCalledTimes(1);
  });

  it("does not run a command when Enter confirms an input-method candidate", () => {
    const { input, onNavigate, onClose } = renderPalette();
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    expect(onNavigate).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("closes on Escape", () => {
    const { input, onClose } = renderPalette();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("asks the workspace to deploy a chosen library prompt", async () => {
    const seen = vi.fn();
    window.addEventListener(DEPLOY_PROMPT_EVENT, (event) => seen((event as CustomEvent<string>).detail), { once: true });
    renderPalette();
    fireEvent.click(await screen.findByTestId("palette-item-prompt-p1"));
    await waitFor(() => expect(seen).toHaveBeenCalledWith("p1"));
  });

  it("asks the workspace to open the composer", async () => {
    const seen = vi.fn();
    window.addEventListener(QUICK_DEPLOY_EVENT, seen, { once: true });
    renderPalette();
    fireEvent.click(screen.getByTestId("palette-item-quick-deploy"));
    await waitFor(() => expect(seen).toHaveBeenCalled());
  });

  it("offers only navigation away from an agent page", () => {
    renderPalette({ page: { kind: "settings" } });
    expect(screen.queryByTestId("palette-item-quick-deploy")).not.toBeInTheDocument();
    expect(listPrompts).not.toHaveBeenCalled();
  });
});
