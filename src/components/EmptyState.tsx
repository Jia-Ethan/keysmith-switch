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
      className="flex flex-col items-center justify-center rounded-lg border border-dashed border-border bg-muted/30 px-4 py-10 text-center"
    >
      <p className="text-[13px] font-medium text-foreground">{title}</p>
      {hint ? (
        <p className="mt-1 max-w-[38ch] text-[12px] leading-snug text-muted-foreground">{hint}</p>
      ) : null}
      {action ? <div className="mt-3">{action}</div> : null}
    </div>
  );
}
