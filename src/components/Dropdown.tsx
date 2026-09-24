import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from "react";
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
 * row cannot be read while the list is open. This keeps the list inside the page
 * instead — same width as its anchor, anchored below it, closed by Escape or by
 * clicking away.
 */
export interface DropdownOption<T extends string> {
  value: T;
  label: ReactNode;
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
  const root = useRef<HTMLDivElement>(null);
  const listId = useId();

  const selected = options.find((option) => option.value === value);

  const close = useCallback((refocus: boolean) => {
    setOpen(false);
    if (refocus) root.current?.querySelector<HTMLButtonElement>("[data-dropdown-trigger]")?.focus();
  }, []);

  useEffect(() => {
    if (!open) return undefined;
    const onPointerDown = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [open]);

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

      {open ? (
        <ul
          id={listId}
          role="listbox"
          aria-label={label}
          data-testid={menuTestId}
          className="animate-disclosure absolute right-0 z-20 mt-1 max-h-64 min-w-full overflow-auto rounded-lg border border-border bg-card p-1 shadow-[0_8px_28px_hsl(var(--shadow)/0.12)]"
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
                    "flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-[14px]",
                    index === active ? "bg-accent text-accent-foreground" : "text-foreground",
                  )}
                >
                  <span className="min-w-0 flex-1 truncate">{option.label}</span>
                  {isSelected ? <IconCheck size={13} className="shrink-0 text-primary" /> : null}
                </button>
              </li>
            );
          })}
        </ul>
      ) : null}
    </div>
  );
}
