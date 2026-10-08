import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ContextMenu, type MenuEntry } from "./ContextMenu";

function entries(run = vi.fn()): MenuEntry[] {
  return [
    { kind: "item", key: "open", label: "Open", run, testId: "m-open" },
    { kind: "item", key: "off", label: "Unavailable", run, disabled: true, note: "Why not", testId: "m-off" },
    { kind: "divider", key: "d" },
    {
      kind: "group",
      key: "to",
      label: "Copy to",
      items: [
        { key: "a", label: "Tool A", run, testId: "m-a" },
        { key: "b", label: "Tool B", run, testId: "m-b" },
      ],
    },
    { kind: "item", key: "delete", label: "Delete", run, danger: true, testId: "m-delete" },
  ];
}

describe("ContextMenu", () => {
  it("runs an item and closes", () => {
    const run = vi.fn();
    const onClose = vi.fn();
    render(<ContextMenu x={10} y={10} label="Actions" entries={entries(run)} onClose={onClose} />);
    expect(screen.getByRole("menu", { name: "Actions" })).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("m-open"));
    expect(run).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalled();
  });

  it("shows why an item is unavailable, and leaves the menu open when it is chosen", () => {
    const run = vi.fn();
    const onClose = vi.fn();
    render(<ContextMenu x={10} y={10} label="Actions" entries={entries(run)} onClose={onClose} />);
    const off = screen.getByTestId("m-off");
    expect(off).toHaveAttribute("aria-disabled", "true");
    expect(off).toHaveTextContent("Why not");
    fireEvent.click(off);
    expect(run).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("names the group its items belong to", () => {
    render(<ContextMenu x={10} y={10} label="Actions" entries={entries()} onClose={vi.fn()} />);
    const group = screen.getByRole("group", { name: "Copy to" });
    expect(group).toContainElement(screen.getByTestId("m-a"));
    expect(group).toContainElement(screen.getByTestId("m-b"));
  });

  it("walks every item with the arrows, the unavailable one included so its note is read", () => {
    render(<ContextMenu x={10} y={10} label="Actions" entries={entries()} onClose={vi.fn()} />);
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(screen.getByTestId("m-open")).toHaveFocus();
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(screen.getByTestId("m-off")).toHaveFocus();
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(screen.getByTestId("m-a")).toHaveFocus();
    fireEvent.keyDown(document.activeElement!, { key: "End" });
    expect(screen.getByTestId("m-delete")).toHaveFocus();
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(screen.getByTestId("m-open")).toHaveFocus();
    fireEvent.keyDown(document.activeElement!, { key: "ArrowUp" });
    expect(screen.getByTestId("m-delete")).toHaveFocus();
  });

  it("closes on Escape and gives the focus back to what opened it", () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <>
        <button type="button">Card</button>
      </>,
    );
    const card = screen.getByRole("button", { name: "Card" });
    card.focus();
    rerender(
      <>
        <button type="button">Card</button>
        <ContextMenu x={10} y={10} label="Actions" entries={entries()} onClose={onClose} />
      </>,
    );
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
    rerender(
      <>
        <button type="button">Card</button>
      </>,
    );
    expect(card).toHaveFocus();
  });

  it("closes on a click or right-click away, without reaching what is underneath", () => {
    const onClose = vi.fn();
    render(<ContextMenu x={10} y={10} label="Actions" entries={entries()} onClose={onClose} />);
    const backdrop = screen.getByTestId("context-menu-backdrop");
    fireEvent.click(backdrop);
    expect(onClose).toHaveBeenCalledTimes(1);
    // A right-click away closes it too, and never brings up the system menu.
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    backdrop.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("hands a right-click away to whatever is under the pointer, as a system menu does", () => {
    const onOther = vi.fn();
    render(
      <>
        <div data-testid="other" onContextMenu={onOther} />
        <ContextMenu x={10} y={10} label="Actions" entries={entries()} onClose={vi.fn()} />
      </>,
    );
    const other = screen.getByTestId("other");
    const original = document.elementFromPoint;
    document.elementFromPoint = (() => other) as typeof document.elementFromPoint;
    try {
      fireEvent.contextMenu(screen.getByTestId("context-menu-backdrop"), { clientX: 120, clientY: 90 });
    } finally {
      document.elementFromPoint = original;
    }
    expect(onOther).toHaveBeenCalledTimes(1);
    expect(onOther.mock.calls[0]![0]).toMatchObject({ clientX: 120, clientY: 90 });
  });

  it("closes on other keys, so a shortcut such as ⌘K is not left behind it", () => {
    const onClose = vi.fn();
    render(<ContextMenu x={10} y={10} label="Actions" entries={entries()} onClose={onClose} />);
    fireEvent.keyDown(document.activeElement!, { key: "Meta" });
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    fireEvent.keyDown(document.activeElement!, { key: "Tab" });
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(document.activeElement!, { key: "k", metaKey: true });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("moves the keyboard highlight with the pointer, and drops it when the pointer leaves", () => {
    render(<ContextMenu x={10} y={10} label="Actions" entries={entries()} onClose={vi.fn()} />);
    fireEvent.mouseEnter(screen.getByTestId("m-a"));
    expect(screen.getByTestId("m-a")).toHaveFocus();
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(screen.getByTestId("m-b")).toHaveFocus();
    fireEvent.mouseLeave(screen.getByRole("menu"));
    expect(screen.getByRole("menu")).toHaveFocus();
    // An unavailable item takes the highlight too, so it never stays on the item left behind.
    fireEvent.mouseEnter(screen.getByTestId("m-open"));
    fireEvent.mouseEnter(screen.getByTestId("m-off"));
    expect(screen.getByTestId("m-off")).toHaveFocus();
  });

  it("closes when the window loses focus or is resized, or the page is scrolled", () => {
    const onClose = vi.fn();
    render(<ContextMenu x={10} y={10} label="Actions" entries={entries()} onClose={onClose} />);
    fireEvent.blur(window);
    fireEvent(window, new Event("resize"));
    fireEvent.wheel(screen.getByTestId("context-menu-backdrop"), { deltaY: 40 });
    expect(onClose).toHaveBeenCalledTimes(3);
  });

  it("stays inside the window near its right and bottom edges", () => {
    render(<ContextMenu x={window.innerWidth - 4} y={window.innerHeight - 4} label="Actions" entries={entries()} onClose={vi.fn()} />);
    const menu = screen.getByRole("menu");
    expect(parseFloat(menu.style.left)).toBeLessThanOrEqual(window.innerWidth - 208 - 8);
    expect(parseFloat(menu.style.top)).toBeLessThanOrEqual(window.innerHeight - 8);
    expect(menu.style.visibility).toBe("visible");
  });

  it("moves up when it grows taller near the bottom edge", () => {
    const { rerender } = render(
      <ContextMenu x={10} y={window.innerHeight - 100} label="Actions" entries={entries()} onClose={vi.fn()} />,
    );
    const menu = screen.getByRole("menu");
    const before = parseFloat(menu.style.top);
    // A note under an item makes it taller; jsdom has no layout, so the new height is given.
    menu.getBoundingClientRect = () => ({ height: 300 }) as DOMRect;
    rerender(<ContextMenu x={10} y={window.innerHeight - 100} label="Actions" entries={entries()} onClose={vi.fn()} />);
    expect(parseFloat(menu.style.top)).toBe(window.innerHeight - 300 - 8);
    expect(parseFloat(menu.style.top)).toBeLessThan(before);
  });
});
