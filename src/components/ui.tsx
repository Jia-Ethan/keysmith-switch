import { forwardRef, useCallback, useId, useLayoutEffect, useRef, useState } from "react";
import type {
  ButtonHTMLAttributes,
  ComponentPropsWithRef,
  InputHTMLAttributes,
  ReactNode,
  RefObject,
  SelectHTMLAttributes,
  TextareaHTMLAttributes,
} from "react";
import { IconChevronRight } from "./icons";

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

const FOCUS_RING =
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background";

const BUTTON_VARIANTS = {
  primary: cx(
    "bg-primary text-primary-foreground",
    "shadow-[inset_0_1px_0_rgb(255_255_255/0.16),0_1px_2px_rgb(var(--primary)/0.35),0_6px_16px_-6px_rgb(var(--primary)/0.55)]",
    "hover:brightness-[1.07] disabled:hover:brightness-100",
  ),
  outline: cx(
    "border border-border bg-card text-foreground shadow-[0_1px_2px_hsl(var(--shadow)/0.05)]",
    "hover:border-foreground/20 hover:bg-accent disabled:hover:bg-card disabled:hover:border-border",
  ),
  subtle: "bg-primary/10 text-primary hover:bg-primary/[0.16] disabled:hover:bg-primary/10",
  ghost: "text-muted-foreground hover:bg-accent hover:text-foreground disabled:hover:bg-transparent",
  danger: cx(
    "border border-destructive/30 bg-destructive/[0.04] text-destructive",
    "hover:border-destructive/50 hover:bg-destructive/10 disabled:hover:bg-transparent",
  ),
} as const;

const BUTTON_SIZES = {
  xs: "h-7 gap-1 rounded-lg px-2.5 text-[12.5px]",
  sm: "h-8 gap-1.5 rounded-lg px-3 text-[13px]",
  md: "h-9 gap-1.5 rounded-[10px] px-3.5 text-[13.5px]",
  lg: "h-10 gap-2 rounded-xl px-4 text-[14px]",
  icon: "h-9 w-9 justify-center rounded-[10px] p-0",
  iconSm: "h-8 w-8 justify-center rounded-lg p-0",
} as const;

export type ButtonVariant = keyof typeof BUTTON_VARIANTS;
export type ButtonSize = keyof typeof BUTTON_SIZES;

export const Button = forwardRef<HTMLButtonElement, ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
}>(function Button({
  variant = "outline",
  size = "md",
  className,
  ...props
}, ref) {
  return (
    <button
      ref={ref}
      type={props.type ?? "button"}
      className={cx(
        "inline-flex shrink-0 items-center whitespace-nowrap font-medium",
        "transition-[background-color,border-color,color,box-shadow,filter,transform] duration-150 active:scale-[0.97] disabled:active:scale-100",
        FOCUS_RING,
        BUTTON_SIZES[size],
        BUTTON_VARIANTS[variant],
        className,
      )}
      {...props}
    />
  );
});

export function IconButton({
  label,
  className,
  children,
  size = "icon",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string; size?: "icon" | "iconSm" }) {
  return (
    <Button
      size={size}
      variant="ghost"
      title={label}
      aria-label={label}
      className={className}
      {...props}
    >
      {children}
    </Button>
  );
}

export function Field({
  label,
  hint,
  children,
  className,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <label className={cx("flex min-w-0 flex-col gap-1.5", className)}>
      <span className="text-[12.5px] font-medium text-muted-foreground">{label}</span>
      {children}
      {hint ? <span className="text-[12.5px] text-muted-foreground">{hint}</span> : null}
    </label>
  );
}

const CONTROL_BASE = cx(
  "w-full rounded-[10px] border border-input bg-card text-foreground shadow-[inset_0_1px_2px_hsl(var(--shadow)/0.04)]",
  "transition-[border-color,box-shadow] placeholder:text-muted-foreground/80 hover:border-foreground/25",
  "focus-visible:outline-none focus-visible:border-primary/60 focus-visible:ring-[3px] focus-visible:ring-primary/15",
);

export function Input({ className, ...props }: ComponentPropsWithRef<"input">) {
  return <input {...props} className={cx(CONTROL_BASE, "h-9 px-3 text-[13.5px]", className)} />;
}

export function Textarea({ className, ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <textarea
      {...props}
      className={cx(
        CONTROL_BASE,
        "resize-y px-3 py-2 font-mono text-[13px] leading-relaxed",
        className,
      )}
    />
  );
}

export function Select({ className, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select {...props} className={cx(CONTROL_BASE, "h-9 px-3 text-[13.5px]", className)} />
  );
}

export function Checkbox({
  label,
  hint,
  className,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & { label?: ReactNode; hint?: ReactNode }) {
  if (label === undefined) {
    return (
      <input
        type="checkbox"
        {...props}
        className={cx("h-4 w-4 shrink-0 accent-[rgb(var(--primary))]", FOCUS_RING, className)}
      />
    );
  }
  return (
    <label className={cx("flex items-start gap-2 text-[14px]", className)}>
      <input
        type="checkbox"
        {...props}
        className={cx("mt-0.5 h-4 w-4 shrink-0 accent-[rgb(var(--primary))]", FOCUS_RING)}
      />
      <span className="min-w-0">
        <span className="block font-medium text-foreground">{label}</span>
        {hint ? <span className="block text-muted-foreground">{hint}</span> : null}
      </span>
    </label>
  );
}

/** Neutral content surface. Used sparingly: one level, no nesting. */
export function Panel({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cx("surface-card", className)}>
      {children}
    </section>
  );
}

export function PanelHeader({
  title,
  actions,
  className,
}: {
  title: ReactNode;
  actions?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cx(
        "flex min-h-[44px] items-center gap-2 border-b border-border px-4 py-2",
        className,
      )}
    >
      <h2 className="min-w-0 truncate text-[15px] font-semibold text-foreground">{title}</h2>
      {actions ? <div className="ml-auto flex shrink-0 items-center gap-1">{actions}</div> : null}
    </div>
  );
}

/** Compact settings row: label + description on the left, control on the right. */
export function SettingRow({
  label,
  description,
  control,
  htmlFor,
}: {
  label: string;
  description?: string;
  control: ReactNode;
  htmlFor?: string;
}) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5 border-b border-border/70 px-5 py-4 last:border-b-0">
      <div className="min-w-[160px] flex-1">
        <label htmlFor={htmlFor} className="block text-[14px] font-medium text-foreground">
          {label}
        </label>
        {description ? (
          <p className="mt-0.5 text-[12.5px] leading-snug text-muted-foreground">{description}</p>
        ) : null}
      </div>
      <div className="flex shrink-0 items-center gap-1.5">{control}</div>
    </div>
  );
}

export function SectionLabel({ children }: { children: ReactNode }) {
  return (
    <h3 className="text-[11.5px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
      {children}
    </h3>
  );
}

/**
 * Keeps an absolutely positioned highlight under the active child of `container`.
 * Measured before paint, re-measured on resize, so the highlight glides between
 * options instead of jumping.
 */
export function useSlidingIndicator(
  container: RefObject<HTMLElement | null>,
  selector: string,
  deps: unknown[],
) {
  const [rect, setRect] = useState<{ left: number; width: number; top: number; height: number } | null>(null);
  const measure = useCallback(() => {
    const root = container.current;
    const active = root?.querySelector<HTMLElement>(selector);
    if (!root || !active || active.offsetWidth === 0) {
      setRect(null);
      return;
    }
    const next = { left: active.offsetLeft, width: active.offsetWidth, top: active.offsetTop, height: active.offsetHeight };
    setRect((current) =>
      current && current.left === next.left && current.width === next.width && current.top === next.top && current.height === next.height
        ? current
        : next,
    );
  }, [container, selector]);

  useLayoutEffect(() => {
    measure();
    const root = container.current;
    if (!root || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(root);
    return () => observer.disconnect();
  }, [measure, ...deps]);

  return rect;
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  className,
  disabled = false,
  size = "md",
}: {
  value: T;
  options: Array<{ value: T; label: ReactNode; title?: string }>;
  onChange: (value: T) => void;
  ariaLabel: string;
  className?: string;
  disabled?: boolean;
  size?: "sm" | "md";
}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const indicator = useSlidingIndicator(rootRef, '[aria-checked="true"]', [value, options.length]);
  return (
    <div
      ref={rootRef}
      role="radiogroup"
      aria-label={ariaLabel}
      className={cx("relative inline-flex gap-0.5 rounded-[10px] bg-muted p-[3px]", className)}
    >
      {indicator ? (
        <span
          aria-hidden="true"
          className="pointer-events-none absolute rounded-[8px] bg-card shadow-[0_1px_2px_hsl(var(--shadow)/0.1),0_0_0_0.5px_hsl(var(--shadow)/0.06)] transition-[left,width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] dark:bg-accent"
          style={{ left: indicator.left, width: indicator.width, top: indicator.top, height: indicator.height }}
        />
      ) : null}
      {options.map((option) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={active}
            title={option.title}
            disabled={disabled}
            onClick={() => onChange(option.value)}
            className={cx(
              "relative z-[1] inline-flex items-center gap-1.5 rounded-[8px] font-medium transition-colors",
              size === "sm" ? "h-7 px-2.5 text-[12.5px]" : "h-8 px-3 text-[13px]",
              FOCUS_RING,
              active
                ? cx("text-foreground", !indicator && "bg-card shadow-sm")
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

/** Collapsed-by-default disclosure for advanced or diagnostic detail. */
export function Disclosure({
  title,
  children,
  defaultOpen = false,
  testId,
}: {
  title: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
  testId?: string;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const id = useId();
  return (
    <div className="rounded-xl border border-border bg-card/60" data-testid={testId}>
      <button
        type="button"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => setOpen((value) => !value)}
        className={cx(
          "flex w-full items-center gap-1.5 rounded-xl px-3 py-2 text-left text-[13px] font-medium text-muted-foreground transition-colors hover:text-foreground",
          FOCUS_RING,
        )}
      >
        <IconChevronRight className={cx("transition-transform duration-200", open && "rotate-90")} />
        <span className="min-w-0 truncate">{title}</span>
      </button>
      {open ? (
        <div id={id} className="animate-disclosure border-t border-border px-3 py-2.5">
          {children}
        </div>
      ) : null}
    </div>
  );
}

export function Mono({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span className={cx("break-all font-mono text-[12px] leading-snug text-muted-foreground", className)}>
      {children}
    </span>
  );
}

export function Tag({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex h-5 max-w-[140px] items-center truncate rounded-md bg-muted px-1.5 text-[11.5px] font-medium text-muted-foreground">
      {children}
    </span>
  );
}
