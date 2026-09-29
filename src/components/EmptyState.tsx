import type { ReactNode } from "react";

export function EmptyState({
  title,
  hint,
  action,
  icon,
  testId = "empty-state",
}: {
  title: string;
  hint?: string;
  action?: ReactNode;
  icon?: ReactNode;
  testId?: string;
}) {
  return (
    <div
      data-testid={testId}
      className="animate-page-in relative flex min-h-[220px] flex-col items-center justify-center overflow-hidden rounded-2xl border border-dashed border-border bg-card/50 px-6 py-12 text-center"
      role="status"
    >
      <div
        aria-hidden="true"
        className="pointer-events-none absolute left-1/2 top-0 h-40 w-72 -translate-x-1/2 rounded-full bg-primary/10 blur-3xl"
      />
      {icon ? (
        <div className="relative mb-4 flex h-12 w-12 items-center justify-center rounded-2xl bg-card text-primary shadow-pop ring-1 ring-border">
          {icon}
        </div>
      ) : null}
      <p className="relative text-[15px] font-semibold text-foreground">{title}</p>
      {hint ? (
        <p className="relative mt-1.5 max-w-[40ch] text-[12.5px] leading-relaxed text-muted-foreground">{hint}</p>
      ) : null}
      {action ? <div className="relative mt-4">{action}</div> : null}
    </div>
  );
}
