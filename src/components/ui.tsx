import { useId, useState } from "react";
import type {
  ButtonHTMLAttributes,
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
  TextareaHTMLAttributes,
} from "react";
import { IconChevronDown, IconChevronRight } from "./icons";

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

const FOCUS_RING =
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background";

const BUTTON_VARIANTS = {
  primary: "bg-primary text-primary-foreground hover:brightness-[1.07] disabled:hover:brightness-100",
  outline:
    "border border-border bg-card text-foreground hover:bg-muted disabled:hover:bg-card",
  ghost: "text-muted-foreground hover:bg-muted hover:text-foreground disabled:hover:bg-transparent",
  danger:
    "border border-destructive/40 bg-transparent text-destructive hover:bg-destructive/10 disabled:hover:bg-transparent",
} as const;

const BUTTON_SIZES = {
  sm: "h-8 gap-1 px-2.5 text-[13px]",
  md: "h-9 gap-1.5 px-3 text-sm",
  icon: "h-9 w-9 justify-center p-0",
} as const;

export type ButtonVariant = keyof typeof BUTTON_VARIANTS;
export type ButtonSize = keyof typeof BUTTON_SIZES;

export function Button({
  variant = "outline",
  size = "md",
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
}) {
  return (
    <button
      type={props.type ?? "button"}
      className={cx(
        "inline-flex shrink-0 items-center rounded-md font-medium transition-colors",
        FOCUS_RING,
        BUTTON_SIZES[size],
        BUTTON_VARIANTS[variant],
        className,
      )}
      {...props}
    />
  );
}

export function IconButton({
  label,
  className,
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <Button
      size="icon"
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
    <label className={cx("flex min-w-0 flex-col gap-1", className)}>
      <span className="text-[11px] font-medium text-muted-foreground">{label}</span>
      {children}
      {hint ? <span className="text-[11px] text-muted-foreground">{hint}</span> : null}
    </label>
  );
}

const CONTROL_BASE = cx(
  "w-full rounded-md border border-input bg-background text-foreground transition-colors",
  "placeholder:text-muted-foreground hover:border-ring/50",
  FOCUS_RING,
);

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={cx(CONTROL_BASE, "h-9 px-2.5 text-sm", className)} />;
}

export function Textarea({ className, ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <textarea
      {...props}
      className={cx(
        CONTROL_BASE,
        "resize-y px-2 py-1.5 font-mono text-[12px] leading-relaxed",
        className,
      )}
    />
  );
}

export function Select({ className, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select {...props} className={cx(CONTROL_BASE, "h-9 px-2 text-sm", className)} />
  );
}

export function Checkbox({
  label,
  hint,
  className,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & { label: ReactNode; hint?: ReactNode }) {
  return (
    <label className={cx("flex items-start gap-2 text-[12px]", className)}>
      <input
        type="checkbox"
        {...props}
        className={cx("mt-0.5 h-3.5 w-3.5 shrink-0 accent-[hsl(var(--primary))]", FOCUS_RING)}
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
    <section className={cx("rounded-lg border border-border bg-card", className)}>
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
        "flex min-h-[36px] items-center gap-2 border-b border-border px-3 py-1.5",
        className,
      )}
    >
      <h2 className="min-w-0 truncate text-sm font-semibold text-foreground">{title}</h2>
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
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5 border-b border-border px-3 py-2.5 last:border-b-0">
      <div className="min-w-[160px] flex-1">
        <label htmlFor={htmlFor} className="block text-sm font-medium text-foreground">
          {label}
        </label>
        {description ? (
          <p className="mt-0.5 text-[11px] leading-snug text-muted-foreground">{description}</p>
        ) : null}
      </div>
      <div className="flex shrink-0 items-center gap-1.5">{control}</div>
    </div>
  );
}

export function SectionLabel({ children }: { children: ReactNode }) {
  return (
    <h3 className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
      {children}
    </h3>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  className,
}: {
  value: T;
  options: Array<{ value: T; label: ReactNode; title?: string }>;
  onChange: (value: T) => void;
  ariaLabel: string;
  className?: string;
}) {
  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      className={cx("inline-flex gap-1 rounded-md border border-border bg-muted p-0.5", className)}
    >
      {options.map((option) => {
        const active = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={active}
            title={option.title}
            onClick={() => onChange(option.value)}
            className={cx(
              "inline-flex h-7 items-center gap-1.5 rounded px-2 text-[12px] font-medium transition-colors",
              FOCUS_RING,
              active
                ? "bg-background text-foreground shadow-sm"
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
    <div className="rounded-md border border-border" data-testid={testId}>
      <button
        type="button"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => setOpen((value) => !value)}
        className={cx(
          "flex w-full items-center gap-1.5 rounded-md px-2.5 py-1.5 text-left text-[12px] font-medium text-muted-foreground transition-colors hover:text-foreground",
          FOCUS_RING,
        )}
      >
        {open ? <IconChevronDown /> : <IconChevronRight />}
        <span className="min-w-0 truncate">{title}</span>
      </button>
      {open ? (
        <div id={id} className="border-t border-border px-2.5 py-2">
          {children}
        </div>
      ) : null}
    </div>
  );
}

export function Mono({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span className={cx("break-all font-mono text-[11px] text-muted-foreground", className)}>
      {children}
    </span>
  );
}

export function Tag({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex max-w-[120px] items-center truncate rounded border border-border bg-muted px-1 py-px text-[10px] text-muted-foreground">
      {children}
    </span>
  );
}
