import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Switch } from "./ui";

describe("Switch", () => {
  it("is a named switch that reports its state", () => {
    render(<Switch checked aria-label="Apply to Codex" onCheckedChange={() => {}} />);
    const control = screen.getByRole("switch", { name: "Apply to Codex" });
    expect(control).toHaveAttribute("aria-checked", "true");
  });

  it("toggles on click and is a focusable native button, so Space and Enter work", () => {
    const onChange = vi.fn();
    render(<Switch checked={false} aria-label="On" onCheckedChange={onChange} />);
    const control = screen.getByRole("switch");
    fireEvent.click(control);
    expect(onChange).toHaveBeenLastCalledWith(true);
    expect(control.tagName).toBe("BUTTON");
    expect(control).toHaveAttribute("type", "button");
    control.focus();
    expect(control).toHaveFocus();
  });

  it("takes its name from a visible label, which also toggles it", () => {
    const onChange = vi.fn();
    render(<Switch checked label="Input rewrite" onCheckedChange={onChange} />);
    const control = screen.getByRole("switch", { name: "Input rewrite" });
    fireEvent.click(screen.getByText("Input rewrite"));
    expect(onChange).toHaveBeenCalledWith(false);
    expect(control).toHaveAttribute("aria-checked", "true");
  });

  it("ignores input while disabled or busy", () => {
    const onChange = vi.fn();
    const { rerender } = render(<Switch checked={false} disabled aria-label="x" onCheckedChange={onChange} />);
    fireEvent.click(screen.getByRole("switch"));
    rerender(<Switch checked={false} busy aria-label="x" onCheckedChange={onChange} />);
    const control = screen.getByRole("switch");
    expect(control).toHaveAttribute("aria-busy", "true");
    fireEvent.click(control);
    expect(onChange).not.toHaveBeenCalled();
  });
});
