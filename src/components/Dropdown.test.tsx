import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Dropdown } from "./Dropdown";

const OPTIONS = [
  { value: "a", label: "Alpha" },
  { value: "b", label: "Beta" },
  { value: "c", label: "Gamma" },
];

function rect(values: Partial<DOMRect>): DOMRect {
  return { x: 0, y: 0, left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0, toJSON: () => ({}), ...values } as DOMRect;
}

function renderDropdown(onChange = vi.fn()) {
  render(
    <div style={{ overflow: "hidden" }} data-testid="clipping-card">
      <Dropdown label="Pick" testId="pick" menuTestId="pick-menu" value="a" options={OPTIONS} onChange={onChange} />
    </div>,
  );
  return onChange;
}

function placeTrigger(box: Partial<DOMRect>) {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(rect(box));
}

describe("Dropdown", () => {
  beforeEach(() => {
    Object.defineProperty(window, "innerHeight", { value: 768, configurable: true });
    Object.defineProperty(window, "innerWidth", { value: 1024, configurable: true });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders its list in the document body so a clipping card cannot cut it off", () => {
    placeTrigger({ left: 800, right: 960, top: 100, bottom: 136, width: 160, height: 36 });
    renderDropdown();
    fireEvent.click(screen.getByTestId("pick"));
    const menu = screen.getByTestId("pick-menu");
    expect(menu.parentElement).toBe(document.body);
    expect(screen.getByTestId("clipping-card")).not.toContainElement(menu);
  });

  it("opens below the trigger and matches its width when there is room", () => {
    placeTrigger({ left: 800, right: 960, top: 100, bottom: 136, width: 160, height: 36 });
    renderDropdown();
    fireEvent.click(screen.getByTestId("pick"));
    const menu = screen.getByTestId("pick-menu");
    expect(menu.style.position).toBe("fixed");
    expect(menu.style.top).toBe("140px");
    expect(menu.style.bottom).toBe("");
    expect(menu.style.minWidth).toBe("160px");
    expect(menu.style.right).toBe("64px");
  });

  it("flips above the trigger near the bottom edge instead of running off-screen", () => {
    placeTrigger({ left: 800, right: 960, top: 720, bottom: 756, width: 160, height: 36 });
    renderDropdown();
    fireEvent.click(screen.getByTestId("pick"));
    const menu = screen.getByTestId("pick-menu");
    expect(menu.style.top).toBe("");
    expect(menu.style.bottom).toBe("52px");
  });

  it("keeps working from the keyboard and closes on outside clicks", () => {
    placeTrigger({ left: 800, right: 960, top: 100, bottom: 136, width: 160, height: 36 });
    const onChange = renderDropdown();
    const trigger = screen.getByTestId("pick");
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    expect(screen.getByTestId("pick-menu")).toBeInTheDocument();
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.keyDown(trigger, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("b");
    expect(screen.queryByTestId("pick-menu")).not.toBeInTheDocument();

    fireEvent.click(trigger);
    expect(screen.getByTestId("pick-menu")).toBeInTheDocument();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByTestId("pick-menu")).not.toBeInTheDocument();
  });

  it("does not close when the pointer goes down on an option in the portal", () => {
    placeTrigger({ left: 800, right: 960, top: 100, bottom: 136, width: 160, height: 36 });
    const onChange = renderDropdown();
    fireEvent.click(screen.getByTestId("pick"));
    const option = screen.getByRole("option", { name: "Gamma" });
    fireEvent.mouseDown(option);
    expect(screen.getByTestId("pick-menu")).toBeInTheDocument();
    fireEvent.click(option);
    expect(onChange).toHaveBeenCalledWith("c");
  });
});
