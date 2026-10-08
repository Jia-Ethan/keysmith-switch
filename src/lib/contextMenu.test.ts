import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { installContextMenuGuard } from "./contextMenu";

function rightClick(target: Element, init: MouseEventInit = {}): boolean {
  target.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true, button: 2, ...init }));
  const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2, ...init });
  target.dispatchEvent(event);
  return event.defaultPrevented;
}

/** Selects the element's text, with a box under (0,0)–(100,20): jsdom has no layout. */
function select(element: Element): Range {
  const range = document.createRange();
  range.selectNodeContents(element);
  range.getClientRects = () => [{ left: 0, right: 100, top: 0, bottom: 20 }] as unknown as DOMRectList;
  document.getSelection()!.addRange(range);
  return range;
}

describe("installContextMenuGuard", () => {
  let uninstall: () => void;

  beforeEach(() => {
    document.body.innerHTML = `
      <main>
        <p id="text">Plain words</p>
        <input id="field" type="text" />
        <input id="box" type="checkbox" />
        <textarea id="area"></textarea>
        <div id="editor" contenteditable="true"><span id="inside">code</span></div>
      </main>`;
    uninstall = installContextMenuGuard(document);
  });

  afterEach(() => {
    uninstall();
    document.getSelection()?.removeAllRanges();
    document.body.innerHTML = "";
  });

  it("hides the system menu (Reload, Share…) on the page", () => {
    expect(rightClick(document.getElementById("text")!)).toBe(true);
    expect(rightClick(document.querySelector("main")!)).toBe(true);
  });

  it("keeps it in text fields, where cut, paste and spelling live", () => {
    expect(rightClick(document.getElementById("field")!)).toBe(false);
    expect(rightClick(document.getElementById("area")!)).toBe(false);
    expect(rightClick(document.getElementById("inside")!)).toBe(false);
  });

  it("does not count a checkbox as a text field", () => {
    expect(rightClick(document.getElementById("box")!)).toBe(true);
  });

  it("keeps it on text the person selected, for Copy", () => {
    const text = document.getElementById("text")!;
    select(text);
    expect(rightClick(text, { clientX: 10, clientY: 10 })).toBe(false);
    expect(document.getSelection()!.isCollapsed).toBe(false);
    // Away from the selection the menu is the app's again.
    expect(rightClick(text, { clientX: 300, clientY: 300 })).toBe(true);
  });

  it("clears the word WebKit selects for its own menu once that menu is not shown", () => {
    const text = document.getElementById("text")!;
    text.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true, button: 2, clientX: 10, clientY: 10 }));
    // WebKit picks the word under the pointer between the press and the menu event.
    select(text);
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2, clientX: 10, clientY: 10 });
    text.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(document.getSelection()!.rangeCount).toBe(0);
  });

  it("leaves a selection alone when the menu comes from the keyboard", () => {
    const text = document.getElementById("text")!;
    select(text);
    // Shift+F10 / the menu key: no press comes first.
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 10, clientY: 10 });
    text.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(document.getSelection()!.isCollapsed).toBe(false);
  });

  it("forgets a press that brought no menu (Control-click on Windows)", () => {
    const text = document.getElementById("text")!;
    select(text);
    text.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true, button: 0, ctrlKey: true, clientX: 10, clientY: 10 }));
    text.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, button: 0, ctrlKey: true, clientX: 10, clientY: 10 }));
    // A later menu from the keyboard is not taken for a right-click on the selection.
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 10, clientY: 10 });
    text.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
  });

  it("leaves the page alone once uninstalled", () => {
    uninstall();
    expect(rightClick(document.getElementById("text")!)).toBe(false);
    uninstall = () => undefined;
  });
});
