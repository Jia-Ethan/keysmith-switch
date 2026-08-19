import type {
  ButtonHTMLAttributes,
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
  TextareaHTMLAttributes,
} from "react";

function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

const btn: Record<string, string> = {
  primary:
    "bg-accent-500 text-white hover:bg-accent-600 disabled:bg-ink-700 disabled:text-ink-200",
  ghost:
    "bg-transparent text-ink-100 hover:bg-ink-800 disabled:text-ink-200",
  danger:
    "bg-rose-800/80 text-white hover:bg-rose-700 disabled:bg-ink-700",
  outline:
    "border border-ink-200/20 bg-ink-800 text-ink-50 hover:border-accent-600 disabled:opacity-50",
};

export function Button({
  variant = "outline",
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: keyof typeof btn }) {
  return (
    <button
      type={props.type ?? "button"}
      className={cx(
        "inline-flex items-center justify-center gap-1 rounded px-2.5 py-1 text-[12px] font-medium transition",
        btn[variant],
        className,
      )}
      {...props}
    />
  );
}

export function Field({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <label className="flex min-w-0 flex-col gap-1 text-[12px] text-ink-200">
      <span>{label}</span>
      {children}
    </label>
  );
}

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      {...props}
      className={cx(
        "h-8 w-full rounded border border-ink-200/15 bg-ink-800 px-2 text-[12px] text-ink-50 outline-none focus:border-accent-600",
        props.className,
      )}
    />
  );
}

export function Textarea(props: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <textarea
      {...props}
      className={cx(
        "min-h-[180px] w-full resize-y rounded border border-ink-200/15 bg-ink-800 px-2 py-1.5 font-mono text-[12px] text-ink-50 outline-none focus:border-accent-600",
        props.className,
      )}
    />
  );
}

export function Select(props: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select
      {...props}
      className={cx(
        "h-8 rounded border border-ink-200/15 bg-ink-800 px-2 text-[12px] text-ink-50 outline-none focus:border-accent-600",
        props.className,
      )}
    />
  );
}

export function Card({
  title,
  children,
  className,
}: {
  title?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cx("rounded-lg border border-ink-200/10 bg-ink-800/70 p-3", className)}>
      {title ? <h2 className="mb-2 text-[13px] font-semibold text-ink-50">{title}</h2> : null}
      {children}
    </section>
  );
}

export { cx };
