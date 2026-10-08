import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cx } from "./ui";

export interface MenuItem {
  key: string;
  label: string;
  icon?: ReactNode;
  run: () => void;
  /** Unavailable right now: shown dimmed, with `note` saying why. */
  disabled?: boolean;
  note?: string;
  danger?: boolean;
  testId?: string;
}

/** What a right-click menu is made of. */
export type MenuEntry =
  | ({ kind: "item" } & MenuItem)
  | { kind: "divider"; key: string }
  /** Items under a caption ("Copy to another tool"). */
  | { kind: "group"; key: string; label: string; items: MenuItem[] };

/** Same width as the prompt page's "more" menu (w-52). */
const MENU_WIDTH = 208;
const VIEWPORT_MARGIN = 8;
const ITEMS = '[role="menuitem"]';
const MOVES = ["ArrowDown", "ArrowUp", "Home", "End"];
/** Keys that leave the menu open: the focused item takes Enter and Space, and modifiers start a chord. */
const PASS = ["Enter", " ", "Shift", "Control", "Alt", "Meta"];

/** Fixed position at the pointer, pulled back inside the window when it would overflow. */
function placeMenu(x: number, y: number, height: number): CSSProperties {
  const left = Math.min(
    Math.max(VIEWPORT_MARGIN, x),
    Math.max(VIEWPORT_MARGIN, window.innerWidth - MENU_WIDTH - VIEWPORT_MARGIN),
  );
  const top = Math.min(
    Math.max(VIEWPORT_MARGIN, y),
    Math.max(VIEWPORT_MARGIN, window.innerHeight - height - VIEWPORT_MARGIN),
  );
  return { position: "fixed", left, top, width: MENU_WIDTH };
}

/**
 * The app's own right-click menu, opened at the pointer.
 *
 * It behaves like a system menu. A transparent layer under it takes the click
 * that dismisses it, so that click never lands on the card or button beneath;
 * a right-click on the layer closes it and opens the menu of whatever is under
 * the pointer instead. Escape, scrolling, a resize, the window losing focus or
 * any other key closes it, and the arrow keys walk the items. The panel is
 * rendered into `document.body` and measured before paint so it stays inside
 * the window.
 *
 * Render it outside the element that opened it: React events from a portal
 * bubble through the component tree, so a right-click on the layer would also
 * reach the opener's own `onContextMenu`.
 */
export function ContextMenu({
  x,
  y,
  entries,
  label,
  onClose,
  testId = "context-menu",
}: {
  x: number;
  y: number;
  entries: MenuEntry[];
  /** Accessible name of the menu, e.g. the prompt it acts on. */
  label: string;
  onClose: () => void;
  testId?: string;
}) {
  const [style, setStyle] = useState<CSSProperties>({ position: "fixed", left: x, top: y, width: MENU_WIDTH, visibility: "hidden" });
  const menu = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;

  // The height decides how far down the window it fits. Measured after every render (and
  // only stored when the spot moves): a note appearing on an item makes the menu taller.
  useLayoutEffect(() => {
    const height = menu.current?.getBoundingClientRect().height ?? 0;
    const next = placeMenu(x, y, height);
    setStyle((current) =>
      current.visibility === "visible" && current.left === next.left && current.top === next.top
        ? current
        : { ...next, visibility: "visible" },
    );
  });

  useEffect(() => {
    // Opened from the keyboard (Shift+F10), focus goes back to the card on close.
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    menu.current?.focus();
    const dismiss = () => close.current();
    const onKeyDown = (event: KeyboardEvent) => {
      if (PASS.includes(event.key)) return;
      // Walked with the arrows, like a system menu; Tab would leave it open behind the focus.
      if (event.key === "Tab") {
        event.preventDefault();
        return;
      }
      if (MOVES.includes(event.key)) {
        event.preventDefault();
        const items = Array.from(menu.current?.querySelectorAll<HTMLElement>(ITEMS) ?? []);
        if (!items.length) return;
        const current = items.indexOf(document.activeElement as HTMLElement);
        const down = event.key === "ArrowDown";
        const next =
          event.key === "Home"
            ? 0
            : event.key === "End"
              ? items.length - 1
              : current < 0
                ? down ? 0 : items.length - 1
                : (current + (down ? 1 : -1) + items.length) % items.length;
        items[next]?.focus();
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
      }
      // Anything else, a shortcut such as ⌘K included, closes it and goes on to the page.
      close.current();
    };
    document.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("resize", dismiss);
    window.addEventListener("blur", dismiss);
    return () => {
      document.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("resize", dismiss);
      window.removeEventListener("blur", dismiss);
      if (opener?.isConnected) opener.focus();
    };
  }, []);

  const renderItem = (item: MenuItem) => (
    <button
      key={item.key}
      type="button"
      role="menuitem"
      tabIndex={-1}
      // Not `disabled`: an unavailable item can still be reached by the arrows and the pointer,
      // so its note is seen and read out, but choosing it does nothing and the menu stays open.
      aria-disabled={item.disabled || undefined}
      data-testid={item.testId}
      // One highlight, like a system menu: the pointer moves the same focus the arrows do.
      onMouseEnter={(event) => event.currentTarget.focus()}
      onClick={() => {
        if (item.disabled) return;
        close.current();
        item.run();
      }}
      className={cx(
        "flex w-full flex-col rounded-lg px-2.5 py-1.5 text-left text-[13px] transition-colors focus:outline-none",
        item.disabled
          ? "cursor-not-allowed text-muted-foreground/70 focus:bg-muted/60"
          : item.danger
            ? "text-destructive focus:bg-destructive/10"
            : "text-foreground focus:bg-muted",
      )}
    >
      <span className="flex w-full items-center gap-2">
        <span className="flex w-3.5 shrink-0 items-center justify-center" aria-hidden="true">
          {item.icon}
        </span>
        <span className="min-w-0 flex-1 truncate">{item.label}</span>
      </span>
      {item.note ? (
        <span className="pl-[22px] pt-0.5 text-[11.5px] leading-snug text-muted-foreground">{item.note}</span>
      ) : null}
    </button>
  );

  return createPortal(
    <>
      <div
        aria-hidden="true"
        data-testid={`${testId}-backdrop`}
        className="fixed inset-0 z-[95]"
        onClick={() => close.current()}
        // A menu pinned to the window would drift off its card as the page moved under it.
        onWheel={() => close.current()}
        onContextMenu={(event) => {
          event.preventDefault();
          close.current();
          // As with a system menu, a right-click elsewhere opens the menu of what is there.
          const layer = event.currentTarget;
          layer.style.pointerEvents = "none";
          const below = document.elementFromPoint?.(event.clientX, event.clientY);
          layer.style.pointerEvents = "";
          below?.dispatchEvent(
            new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: event.clientX, clientY: event.clientY }),
          );
        }}
      />
      <div
        ref={menu}
        role="menu"
        aria-label={label}
        tabIndex={-1}
        data-testid={testId}
        style={style}
        onContextMenu={(event) => event.preventDefault()}
        // Leaving the menu drops the highlight, as a system menu does.
        onMouseLeave={() => menu.current?.focus()}
        className="animate-disclosure z-[96] max-h-[80vh] overflow-auto rounded-xl border border-border bg-card p-1 shadow-pop focus:outline-none"
      >
        {entries.map((entry) => {
          if (entry.kind === "divider") {
            return <div key={entry.key} role="separator" className="my-1 border-t border-border" />;
          }
          if (entry.kind === "group") {
            return (
              <div key={entry.key} role="group" aria-label={entry.label}>
                <p
                  aria-hidden="true"
                  className="px-2.5 py-1.5 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground"
                >
                  {entry.label}
                </p>
                {entry.items.map(renderItem)}
              </div>
            );
          }
          return renderItem(entry);
        })}
      </div>
    </>,
    document.body,
  );
}
