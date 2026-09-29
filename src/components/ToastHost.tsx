import { useTranslation } from "react-i18next";
import { TOAST_DURATION_MS, type ToastApi } from "../hooks/useToasts";
import { IconAlert, IconCheck, IconClose, IconInfo } from "./icons";
import { cx, IconButton } from "./ui";

const TOAST_BADGE = {
  info: "bg-muted text-muted-foreground",
  ok: "bg-success/15 text-success",
  err: "bg-destructive/15 text-destructive",
} as const;

const TOAST_BAR = {
  info: "bg-muted-foreground/40",
  ok: "bg-success/60",
  err: "bg-destructive/60",
} as const;

const TOAST_ICON = {
  info: IconInfo,
  ok: IconCheck,
  err: IconAlert,
} as const;

export function ToastHost({ toasts, dismiss }: Pick<ToastApi, "toasts" | "dismiss">) {
  const { t } = useTranslation();
  if (toasts.length === 0) return null;
  return (
    <div className="pointer-events-none fixed right-4 top-[64px] z-[80] flex w-[min(22rem,calc(100vw-2rem))] flex-col gap-2">
      {toasts.map((toast) => {
        const Icon = TOAST_ICON[toast.kind];
        return (
          <div
            key={toast.id}
            role="status"
            aria-live="polite"
            data-kind={toast.kind}
            className="animate-toast-in glass pointer-events-auto relative flex items-start gap-2.5 overflow-hidden rounded-2xl border border-border px-3 py-2.5 text-[13px] text-foreground shadow-pop"
          >
            <span className={cx("mt-px flex h-6 w-6 shrink-0 items-center justify-center rounded-full", TOAST_BADGE[toast.kind])}>
              <Icon size={13} />
            </span>
            <p className="min-w-0 flex-1 break-words py-0.5 leading-snug">{toast.message}</p>
            <IconButton
              label={t("common.close")}
              onClick={() => dismiss(toast.id)}
              className="h-6 w-6 shrink-0 rounded-md"
            >
              <IconClose size={12} />
            </IconButton>
            <span
              aria-hidden="true"
              className={cx("toast-timer absolute inset-x-0 bottom-0 h-[2px]", TOAST_BAR[toast.kind])}
              style={{ animationDuration: `${TOAST_DURATION_MS}ms` }}
            />
          </div>
        );
      })}
    </div>
  );
}
