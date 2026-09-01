import type { ReactNode } from "react";

export function EmptyState({
  title,
  hint,
  action,
  testId = "empty-state",
}: {
  title: string;
  hint?: string;
  action?: ReactNode;
  testId?: string;
}) {
  return (
    <div
      data-testid={testId}
      className="flex min-h-[180px] flex-col items-center justify-center rounded-2xl border border-dashed border-border bg-muted/25 px-6 py-12 text-center"
      role="status"
    >
      <p className="text-[17px] font-medium text-foreground">{title}</p>
      {hint ? (
        <p className="mt-1.5 max-w-[38ch] text-[14px] leading-snug text-muted-foreground">{hint}</p>
      ) : null}
      {action ? <div className="mt-3">{action}</div> : null}
    </div>
  );
}
