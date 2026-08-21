import { useTranslation } from "react-i18next";
import type { ToastApi } from "../hooks/useToasts";
import { IconAlert, IconCheck, IconClose, IconInfo } from "./icons";
import { cx, IconButton } from "./ui";

const TOAST_TONE = {
  info: "border-border bg-card text-foreground",
  ok: "border-primary/40 bg-primary/10 text-primary",
  err: "border-destructive/40 bg-destructive/10 text-destructive",
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
    <div className="pointer-events-none fixed right-4 top-[4.5rem] z-50 flex w-[min(20rem,calc(100vw-2rem))] flex-col gap-2">
      {toasts.map((toast) => {
        const Icon = TOAST_ICON[toast.kind];
        return (
          <div
            key={toast.id}
            role="status"
            aria-live="polite"
            className={cx(
              "pointer-events-auto flex items-start gap-2 rounded-xl border px-3 py-2.5 text-[12px] shadow-lg",
              TOAST_TONE[toast.kind],
            )}
          >
            <Icon size={14} className="mt-px shrink-0" />
            <p className="min-w-0 flex-1 break-words leading-snug">{toast.message}</p>
            <IconButton
              label={t("common.close")}
              onClick={() => dismiss(toast.id)}
              className="h-5 w-5 shrink-0 hover:bg-transparent hover:opacity-70"
            >
              <IconClose size={12} />
            </IconButton>
          </div>
        );
      })}
    </div>
  );
}
