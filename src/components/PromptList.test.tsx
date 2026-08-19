import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PromptList } from "./PromptList";

describe("PromptList empty state", () => {
  it("renders the empty state when there are no prompts", () => {
    render(<PromptList prompts={[]} selectedId={null} onSelect={() => undefined} />);
    expect(screen.getByTestId("prompt-list-empty")).toBeInTheDocument();
    expect(screen.getByText("还没有提示词")).toBeInTheDocument();
  });
});
