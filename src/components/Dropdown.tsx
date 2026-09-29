import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { IconCheck, IconChevronDown } from "./icons";
import { cx } from "./ui";

/** Same ring treatment as the shared controls in ui.tsx, kept local to that module's private helper. */
const FOCUS_RING =
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background";

/**
 * An in-page dropdown for short option lists.
 *
 * The native `<select>` on desktop platforms opens a system overlay: it paints
 * over neighbouring controls and gives the choice its own look, so a settings
 * row cannot be read while the list is open. This keeps the list in the page
 * instead — at least as wide as its anchor, closed by Escape or by clicking away.
 *
 * The list is rendered into `document.body` and positioned against the viewport.
 * Settings rows sit in cards that clip their overflow, and an absolutely
 * positioned list inside one is cut off at the card's edge. It opens below the
 * trigger, or above it when there is more room there.
 */
export interface DropdownOption<T extends string> {
  value: T;
  label: ReactNode;
}

const MENU_GAP = 4;
const VIEWPORT_MARGIN = 8;
const MENU_MAX_HEIGHT = 256;
/** p-1 padding plus one 34px row per option. */
const menuHeightFor = (count: number) => Math.min(MENU_MAX_HEIGHT, count * 34 + 8);

/** Fixed-position style that keeps the list inside the window, flipping when it does not fit below. */
function placeMenu(trigger: HTMLElement, count: number): CSSProperties {
  const rect = trigger.getBoundingClientRect();
  const wanted = menuHeightFor(count);
  const below = window.innerHeight - rect.bottom - MENU_GAP - VIEWPORT_MARGIN;
  const above = rect.top - MENU_GAP - VIEWPORT_MARGIN;
  const flip = below < wanted && above > below;
  const room = Math.max(96, Math.min(MENU_MAX_HEIGHT, flip ? above : below));
  return {
    position: "fixed",
    right: Math.max(VIEWPORT_MARGIN, window.innerWidth - rect.right),
    minWidth: rect.width,
    maxHeight: room,
    ...(flip
      ? { bottom: window.innerHeight - rect.top + MENU_GAP }
      : { top: rect.bottom + MENU_GAP }),
  };
}

export function Dropdown<T extends string>({
  label,
  value,
  options,
  disabled = false,
  onChange,
  testId,
  menuTestId,
  optionTestId,
  className,
}: {
  /** Accessible name of both the trigger and the listbox. */
  label: string;
  value: T;
  options: Array<DropdownOption<T>>;
  disabled?: boolean;
  onChange: (value: T) => void;
  testId?: string;
  menuTestId?: string;
  optionTestId?: (value: T) => string;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(() => Math.max(0, options.findIndex((o) => o.value === value)));
  const [menuStyle, setMenuStyle] = useState<CSSProperties | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const menu = useRef<HTMLUListElement>(null);
  const listId = useId();

  const selected = options.find((option) => option.value === value);

  const close = useCallback((refocus: boolean) => {
    setOpen(false);
    if (refocus) root.current?.querySelector<HTMLButtonElement>("[data-dropdown-trigger]")?.focus();
  }, []);

  useEffect(() => {
    if (!open) return undefined;
    const onPointerDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!root.current?.contains(target) && !menu.current?.contains(target)) setOpen(false);
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [open]);

  // Measure before paint, then follow the trigger while the page scrolls or resizes.
  useLayoutEffect(() => {
    if (!open) {
      setMenuStyle(null);
      return undefined;
    }
    const trigger = root.current?.querySelector<HTMLElement>("[data-dropdown-trigger]");
    if (!trigger) return undefined;
    const update = () => setMenuStyle(placeMenu(trigger, options.length));
    update();
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
    };
  }, [open, options.length]);

  const openList = () => {
    setActive(Math.max(0, options.findIndex((option) => option.value === value)));
    setOpen(true);
  };

  const commit = (next: T) => {
    close(true);
    if (next !== value) onChange(next);
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (!open) {
      if (event.key === "Enter" || event.key === " " || event.key === "ArrowDown") {
        event.preventDefault();
        openList();
      }
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      close(true);
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      setActive((index) => (index + 1) % options.length);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActive((index) => (index - 1 + options.length) % options.length);
    } else if (event.key === "Home") {
      event.preventDefault();
      setActive(0);
    } else if (event.key === "End") {
      event.preventDefault();
      setActive(options.length - 1);
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      const option = options[active];
      if (option) commit(option.value);
    }
  };

  return (
    <div ref={root} className={cx("relative", className)} onKeyDown={onKeyDown}>
      <button
        type="button"
        data-dropdown-trigger=""
        data-testid={testId}
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        disabled={disabled}
        onClick={() => (open ? close(false) : openList())}
        className={cx(
          "flex h-9 w-full items-center justify-between gap-2 rounded-lg border border-input bg-background px-3 text-[14px] text-foreground",
          "transition-colors hover:border-ring/60 disabled:cursor-not-allowed disabled:opacity-60",
          FOCUS_RING,
        )}
      >
        <span className="truncate">{selected?.label ?? ""}</span>
        <IconChevronDown
          size={13}
          className={cx("shrink-0 text-muted-foreground transition-transform", open && "rotate-180")}
        />
      </button>

      {open && menuStyle
        ? createPortal(
            <ul
              ref={menu}
              id={listId}
              role="listbox"
              aria-label={label}
              data-testid={menuTestId}
              style={menuStyle}
              className="animate-disclosure z-[90] overflow-auto rounded-lg border border-border bg-card p-1 shadow-[0_8px_28px_hsl(var(--shadow)/0.16)]"
            >
              {options.map((option, index) => {
                const isSelected = option.value === value;
                return (
                  <li key={option.value}>
                    <button
                      type="button"
                      role="option"
                      aria-selected={isSelected}
                      data-testid={optionTestId?.(option.value)}
                      onClick={() => commit(option.value)}
                      onMouseEnter={() => setActive(index)}
                      className={cx(
                        "flex h-[34px] w-full items-center gap-2 rounded-md px-2.5 text-left text-[14px]",
                        index === active ? "bg-accent text-accent-foreground" : "text-foreground",
                      )}
                    >
                      <span className="min-w-0 flex-1 truncate">{option.label}</span>
                      {isSelected ? <IconCheck size={13} className="shrink-0 text-primary" /> : null}
                    </button>
                  </li>
                );
              })}
            </ul>,
            document.body,
          )
        : null}
    </div>
  );
}
