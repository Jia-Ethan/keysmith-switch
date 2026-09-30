import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "./ui";

describe("Button loading state", () => {
  it("swaps the icon for a spinner, reports busy, and ignores clicks", () => {
    const onClick = vi.fn();
    render(
      <Button loading onClick={onClick}>
        <svg data-testid="icon" />
        Deploy
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Deploy" });
    expect(button).toHaveAttribute("aria-busy", "true");
    expect(button.querySelector(".spinner")).toBeInTheDocument();
    expect(button.className).toContain("[&>svg]:hidden");
    fireEvent.click(button);
    expect(onClick).not.toHaveBeenCalled();
  });

  it("behaves as a plain button when it is not loading", () => {
    const onClick = vi.fn();
    render(
      <Button onClick={onClick}>
        <svg data-testid="icon" />
        Deploy
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Deploy" });
    expect(button).not.toHaveAttribute("aria-busy");
    expect(button.querySelector(".spinner")).not.toBeInTheDocument();
    fireEvent.click(button);
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
