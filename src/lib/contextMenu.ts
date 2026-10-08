/**
 * The WebView's own context menu belongs to the browser, not to this app. On
 * macOS it offers "Reload" and "Share…" over any part of the window, and Reload
 * throws the page away: the Quick Deploy draft and an unsaved editor live only
 * in memory, so a mis-click there loses work without a warning. Windows
 * (WebView2) has the same reload entry. The app draws its own menus in the page,
 * so the native one is kept only where its items are the useful ones: inside a
 * text field (cut, paste, spelling) and on text the person selected (copy).
 */

const TEXT_INPUT_TYPES = new Set(["text", "search", "url", "email", "password", "number", "tel"]);

function inTextEntry(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  if (target.closest('textarea, [contenteditable]:not([contenteditable="false"])')) return true;
  const input = target.closest("input");
  return input !== null && TEXT_INPUT_TYPES.has(input.type);
}

function onSelection(root: Document, x: number, y: number): boolean {
  const selection = root.getSelection();
  if (!selection || selection.isCollapsed) return false;
  for (let index = 0; index < selection.rangeCount; index += 1) {
    for (const rect of Array.from(selection.getRangeAt(index).getClientRects())) {
      if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) return true;
    }
  }
  return false;
}

/**
 * Installs the guard and returns its uninstaller. Menus inside the app open from
 * their own `contextmenu` handlers and are not affected.
 */
export function installContextMenuGuard(root: Document = document): () => void {
  // Where the press that opens the menu landed. It is read at the press because by the time
  // the menu event arrives, WebKit has already selected the word under the pointer for its
  // own menu, and that word would pass for a selection the person made.
  let press: "none" | "plain" | "selection" = "none";
  const onMouseDown = (event: MouseEvent) => {
    // A secondary press, or Control-click on a Mac: a menu event follows it.
    const opensMenu = event.button === 2 || (event.button === 0 && event.ctrlKey);
    press = !opensMenu ? "none" : onSelection(root, event.clientX, event.clientY) ? "selection" : "plain";
  };
  const onContextMenu = (event: MouseEvent) => {
    const pressed = press;
    press = "none";
    if (inTextEntry(event.target) || pressed === "selection") return;
    event.preventDefault();
    // The word WebKit picked for the menu that is no longer shown would stay highlighted,
    // under the app's own menu too.
    if (pressed === "plain" && onSelection(root, event.clientX, event.clientY)) root.getSelection()?.removeAllRanges();
  };
  // A press that brought no menu (Control-click on Windows) must not count for the next one,
  // which may come from the keyboard.
  const forget = () => {
    press = "none";
  };
  root.addEventListener("mousedown", onMouseDown, true);
  root.addEventListener("click", forget, true);
  root.addEventListener("keydown", forget, true);
  root.addEventListener("contextmenu", onContextMenu);
  return () => {
    root.removeEventListener("mousedown", onMouseDown, true);
    root.removeEventListener("click", forget, true);
    root.removeEventListener("keydown", forget, true);
    root.removeEventListener("contextmenu", onContextMenu);
  };
}
