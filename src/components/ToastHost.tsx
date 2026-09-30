import { useTranslation } from "react-i18next";
import type { ToastApi } from "../hooks/useToasts";
import { IconAlert, IconCheck, IconClose, IconInfo } from "./icons";
import { cx } from "./ui";

const TOAST_BADGE = {
  info: "bg-muted text-muted-foreground",
  ok: "bg-success/15 text-success",
  err: "bg-destructive/15 text-destructive",
} as const;

const TOAST_ICON = {
  info: IconInfo,
  ok: IconCheck,
  err: IconAlert,
} as const;

/**
 * Transient messages sit at the bottom centre: the header and the page's own
 * controls stay clear, and a message is read where the eye already rests after
 * clicking. Each one is a single compact line that rises in and fades out.
 */
export function ToastHost({ toasts, dismiss }: Pick<ToastApi, "toasts" | "dismiss">) {
  const { t } = useTranslation();
  if (toasts.length === 0) return null;
  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-5 z-[80] flex flex-col items-center gap-2 px-4">
      {toasts.map((toast) => {
        const Icon = TOAST_ICON[toast.kind];
        return (
          <div
            key={toast.id}
            role="status"
            aria-live="polite"
            data-kind={toast.kind}
            className="animate-toast-in pointer-events-auto flex max-w-[min(32rem,100%)] items-center gap-2 rounded-full border border-border bg-card/95 py-1.5 pl-1.5 pr-1.5 text-[13px] text-foreground shadow-pop backdrop-blur"
          >
            <span className={cx("flex h-6 w-6 shrink-0 items-center justify-center rounded-full", TOAST_BADGE[toast.kind])}>
              <Icon size={13} />
            </span>
            <p className="min-w-0 break-words py-0.5 pl-0.5 leading-snug">{toast.message}</p>
            <button
              type="button"
              aria-label={t("common.close")}
              title={t("common.close")}
              onClick={() => dismiss(toast.id)}
              className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <IconClose size={12} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
