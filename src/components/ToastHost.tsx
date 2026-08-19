import type { ToastApi } from "../hooks/useToasts";
import { cx } from "./ui";

export function ToastHost({ toasts, dismiss }: Pick<ToastApi, "toasts" | "dismiss">) {
  if (toasts.length === 0) return null;
  return (
    <div className="pointer-events-none fixed right-3 top-12 z-50 flex w-80 flex-col gap-2">
      {toasts.map((toast) => (
        <button
          key={toast.id}
          type="button"
          onClick={() => dismiss(toast.id)}
          className={cx(
            "pointer-events-auto rounded border px-3 py-2 text-left text-[12px]",
            toast.kind === "err" && "border-rose-700/50 bg-rose-950/90 text-rose-100",
            toast.kind === "ok" && "border-teal-700/40 bg-teal-950/90 text-teal-100",
            toast.kind === "info" && "border-ink-200/20 bg-ink-800 text-ink-50",
          )}
        >
          {toast.message}
        </button>
      ))}
    </div>
  );
}
