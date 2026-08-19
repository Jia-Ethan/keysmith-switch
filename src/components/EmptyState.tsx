export function EmptyState({
  title,
  hint,
  testId = "prompt-list-empty",
}: {
  title: string;
  hint?: string;
  testId?: string;
}) {
  return (
    <div
      data-testid={testId}
      className="rounded border border-dashed border-ink-200/15 px-3 py-8 text-center text-ink-200"
    >
      <p className="text-[13px] font-medium text-ink-50">{title}</p>
      {hint ? <p className="mt-1 text-[12px]">{hint}</p> : null}
    </div>
  );
}
