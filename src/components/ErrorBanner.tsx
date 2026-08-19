export function ErrorBanner({
  message,
  onRetry,
  retryLabel,
}: {
  message: string;
  onRetry?: () => void;
  retryLabel?: string;
}) {
  if (!message) return null;
  return (
    <div
      className="flex items-start justify-between gap-3 rounded border border-rose-800/50 bg-rose-950/40 px-3 py-2 text-[12px] text-rose-100"
      role="alert"
    >
      <p className="min-w-0 break-words">{message}</p>
      {onRetry ? (
        <button type="button" className="shrink-0 underline" onClick={onRetry}>
          {retryLabel ?? "Retry"}
        </button>
      ) : null}
    </div>
  );
}
