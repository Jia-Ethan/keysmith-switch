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
      className="flex items-start justify-between gap-3 rounded-xl border border-destructive/45 bg-destructive/10 px-3.5 py-2.5 text-[12px] text-destructive"
      role="alert"
    >
      <p className="min-w-0 break-words">{message}</p>
      {onRetry ? (
        <button
          type="button"
          className="shrink-0 font-medium underline transition-opacity hover:opacity-80"
          onClick={onRetry}
        >
          {retryLabel ?? "Retry"}
        </button>
      ) : null}
    </div>
  );
}
