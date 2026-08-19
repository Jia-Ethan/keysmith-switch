import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
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

    const activeGroup = screen.getByText("已激活提示词").closest("section");
    const inactiveGroup = screen.getByText("未激活提示词").closest("section");
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
    expect(screen.queryByText("已激活提示词")).not.toBeInTheDocument();
    expect(screen.queryByText("未激活提示词")).not.toBeInTheDocument();
    expect(screen.getByTestId("prompt-item-a")).toBeInTheDocument();
    expect(screen.getByTestId("prompt-item-b")).toBeInTheDocument();
  });
});
