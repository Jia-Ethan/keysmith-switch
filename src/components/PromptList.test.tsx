import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PromptList } from "./PromptList";
import type { PromptSummary } from "../types";

function prompt(overrides: Partial<PromptSummary>): PromptSummary {
  return {
    id: "p1",
    tool: "claude",
    title: "Prompt one",
    tags: [],
    active: false,
    lastUsedAt: null,
    updatedAt: "2026-01-01T00:00:00Z",
    createdAt: "2026-01-01T00:00:00Z",
    excerpt: null,
    ...overrides,
  };
}

describe("PromptList empty state", () => {
  it("renders the empty state when there are no prompts", () => {
    render(<PromptList prompts={[]} selectedId={null} onSelect={() => undefined} />);
    expect(screen.getByTestId("prompt-list-empty")).toBeInTheDocument();
    expect(screen.getByText("还没有提示词")).toBeInTheDocument();
  });

  it("distinguishes a filtered miss from an empty library", () => {
    render(<PromptList prompts={[]} selectedId={null} filtered onSelect={() => undefined} />);
    expect(screen.getByTestId("prompt-list-no-results")).toBeInTheDocument();
    expect(screen.queryByTestId("prompt-list-empty")).not.toBeInTheDocument();
  });
});

describe("PromptList activation grouping", () => {
  const prompts = [prompt({ id: "a", title: "Alpha" }), prompt({ id: "b", title: "Beta" })];

  it("groups by the scoped activeIds rather than the per-row flag", () => {
    // The per-row flag claims both are active (it is scope-blind); the scoped
    // context says only "a" is active here.
    const scopeBlind = [
      prompt({ id: "a", title: "Alpha", active: true }),
      prompt({ id: "b", title: "Beta", active: true }),
    ];
    render(
      <PromptList
        prompts={scopeBlind}
        selectedId={null}
        activeIds={["a"]}
        onSelect={() => undefined}
      />,
    );

    const activeGroup = screen.getByText("当前部署").closest("section");
    const inactiveGroup = screen.getByText("提示词库").closest("section");
    expect(activeGroup).not.toBeNull();
    expect(inactiveGroup).not.toBeNull();

    expect(activeGroup).toContainElement(screen.getByTestId("prompt-item-a"));
    expect(activeGroup).not.toContainElement(screen.getByTestId("prompt-item-b"));
    expect(inactiveGroup).toContainElement(screen.getByTestId("prompt-item-b"));
  });

  it("hides Active / Inactive grouping when activation state is unreadable", () => {
    render(
      <PromptList prompts={prompts} selectedId={null} activeIds={null} onSelect={() => undefined} />,
    );
    // Regression guard: a failed activation read must not silently file every
    // prompt as inactive, which reads as "nothing is applied".
    expect(screen.getByTestId("prompt-activation-unknown")).toBeInTheDocument();
    expect(screen.getByText("全部提示词")).toBeInTheDocument();
    expect(screen.queryByText("当前部署")).not.toBeInTheDocument();
    expect(screen.queryByText("提示词库")).not.toBeInTheDocument();
    expect(screen.getByTestId("prompt-item-a")).toBeInTheDocument();
    expect(screen.getByTestId("prompt-item-b")).toBeInTheDocument();
  });
});

describe("PromptList unrecorded live prompt", () => {
  it("shows a live card instead of the empty state when the machine runs an unlisted prompt", () => {
    render(<PromptList prompts={[]} selectedId={null} unrecordedLive onSelect={() => undefined} />);
    const card = screen.getByTestId("prompt-live-unrecorded");
    expect(card).toHaveTextContent("部署中");
    expect(screen.getByText("当前部署").closest("section")).toContainElement(card);
    expect(screen.queryByTestId("prompt-list-empty")).not.toBeInTheDocument();
  });

  it("keeps the live card above the library when other prompts exist", () => {
    render(
      <PromptList
        prompts={[prompt({ id: "a", title: "Alpha" })]}
        selectedId={null}
        activeIds={[]}
        unrecordedLive
        onSelect={() => undefined}
      />,
    );
    expect(screen.getByTestId("prompt-live-unrecorded")).toBeInTheDocument();
    expect(screen.getByText("提示词库").closest("section")).toContainElement(screen.getByTestId("prompt-item-a"));
  });

  it("hides the live card while a search or tag filter is applied", () => {
    render(<PromptList prompts={[]} selectedId={null} filtered unrecordedLive onSelect={() => undefined} />);
    expect(screen.queryByTestId("prompt-live-unrecorded")).not.toBeInTheDocument();
    expect(screen.getByTestId("prompt-list-no-results")).toBeInTheDocument();
  });

  it("badges a matched live prompt as deploying", () => {
    render(
      <PromptList
        prompts={[prompt({ id: "a", title: "Alpha" })]}
        selectedId={null}
        activeIds={["a"]}
        onSelect={() => undefined}
      />,
    );
    expect(screen.getByTestId("prompt-item-a")).toHaveTextContent("部署中");
    expect(screen.queryByTestId("prompt-live-unrecorded")).not.toBeInTheDocument();
  });
});

describe("PromptList card and deploy button", () => {
  const items = [prompt({ id: "a", title: "Alpha" }), prompt({ id: "b", title: "Beta" })];
  const renderList = (extra: Record<string, unknown> = {}) =>
    render(<PromptList prompts={items} selectedId={null} activeIds={[]} onSelect={() => undefined} onDeploy={() => undefined} {...extra} />);

  it("stacks the deploy button above the card's own content", () => {
    // The card's children sit at z-index 1 (for the spotlight); a button below that would be
    // covered, and clicks would land on the card instead.
    renderList();
    expect(screen.getByTestId("prompt-deploy-a").className).toContain("z-10");
  });

  it("lifts the card with the whole row, never with the card alone", () => {
    // `hover:` on the card made it jump away from a pointer resting on the button (a sibling),
    // which dropped the hover and brought it back, every frame.
    renderList();
    const card = screen.getByTestId("prompt-item-a").className;
    expect(card).toContain("group-hover:-translate-y-0.5");
    expect(card.split(/\s+/)).not.toContain("hover:-translate-y-0.5");
  });

  it("keeps the card whose deploy sheet is open in its engaged look", () => {
    renderList({ engagedId: "a" });
    expect(screen.getByTestId("prompt-item-a").closest("li")).toHaveAttribute("data-engaged");
    expect(screen.getByTestId("prompt-item-b").closest("li")).not.toHaveAttribute("data-engaged");
    expect(screen.getByTestId("prompt-deploy-a").className.split(/\s+/)).toContain("opacity-100");
    expect(screen.getByTestId("prompt-deploy-a").className).not.toContain("sm:opacity-0");
    expect(screen.getByTestId("prompt-deploy-b").className).toContain("sm:opacity-0");
  });
});

describe("PromptList extension-pack prompts", () => {
  it("shows the title but never the text, and does not open", () => {
    const onSelect = vi.fn();
    const onDeploy = vi.fn();
    render(
      <PromptList
        prompts={[prompt({ id: "x", title: "Pack prompt", excerpt: "secret first line", locked: true })]}
        selectedId={null}
        onSelect={onSelect}
        onDeploy={onDeploy}
      />,
    );
    expect(screen.getByText("Pack prompt")).toBeInTheDocument();
    expect(screen.queryByText("secret first line")).not.toBeInTheDocument();
    expect(screen.getByTestId("prompt-locked-x")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("prompt-item-x"));
    expect(onSelect).not.toHaveBeenCalled();
    // Deploying it is still allowed.
    fireEvent.click(screen.getByTestId("prompt-deploy-x"));
    expect(onDeploy).toHaveBeenCalledWith("x");
  });

  it("still opens the person's own prompts", () => {
    const onSelect = vi.fn();
    render(<PromptList prompts={[prompt({ id: "m", title: "Mine" })]} selectedId={null} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId("prompt-item-m"));
    expect(onSelect).toHaveBeenCalledWith("m");
  });
});

describe("PromptList card menu", () => {
  it("asks the page for its menu at the pointer, in place of the system one", () => {
    const onCardMenu = vi.fn();
    render(
      <PromptList
        prompts={[prompt({ id: "a", title: "Alpha" }), prompt({ id: "b", title: "Beta" })]}
        selectedId={null}
        activeIds={["a"]}
        onSelect={() => undefined}
        onDeploy={() => undefined}
        onCardMenu={onCardMenu}
      />,
    );
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 40, clientY: 50 });
    screen.getByTestId("prompt-item-b").dispatchEvent(event);
    // The system menu (Reload, Share…) does not show over a card.
    expect(event.defaultPrevented).toBe(true);
    expect(onCardMenu).toHaveBeenLastCalledWith(expect.objectContaining({ id: "b" }), { x: 40, y: 50 });
    // The live card, and the deploy button on a card, open it too.
    fireEvent.contextMenu(screen.getByTestId("prompt-item-a"), { clientX: 5, clientY: 6 });
    expect(onCardMenu).toHaveBeenLastCalledWith(expect.objectContaining({ id: "a" }), { x: 5, y: 6 });
    fireEvent.contextMenu(screen.getByTestId("prompt-deploy-b"), { clientX: 7, clientY: 8 });
    expect(onCardMenu).toHaveBeenLastCalledWith(expect.objectContaining({ id: "b" }), { x: 7, y: 8 });
  });

  it("opens it for pack prompts too, which do not open on click", () => {
    const onCardMenu = vi.fn();
    render(
      <PromptList
        prompts={[prompt({ id: "x", locked: true })]}
        selectedId={null}
        onSelect={() => undefined}
        onCardMenu={onCardMenu}
      />,
    );
    fireEvent.contextMenu(screen.getByTestId("prompt-item-x"), { clientX: 1, clientY: 1 });
    expect(onCardMenu).toHaveBeenCalledWith(expect.objectContaining({ id: "x" }), { x: 1, y: 1 });
  });

  it("opens it in the single list shown when activation state is unknown", () => {
    const onCardMenu = vi.fn();
    render(<PromptList prompts={[prompt({ id: "a" })]} selectedId={null} activeIds={null} onSelect={() => undefined} onCardMenu={onCardMenu} engagedId="a" />);
    fireEvent.contextMenu(screen.getByTestId("prompt-item-a"), { clientX: 2, clientY: 3 });
    expect(onCardMenu).toHaveBeenCalledWith(expect.objectContaining({ id: "a" }), { x: 2, y: 3 });
    expect(screen.getByTestId("prompt-item-a").closest("li")).toHaveAttribute("data-engaged");
  });

  it("leaves right-click alone when the page offers no menu", () => {
    render(<PromptList prompts={[prompt({ id: "a" })]} selectedId={null} activeIds={[]} onSelect={() => undefined} />);
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    screen.getByTestId("prompt-item-a").dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });
});
