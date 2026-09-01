import { useTranslation } from "react-i18next";
import type { ToolStatusName } from "../types";
import { cx } from "./ui";

const STATUS_TONE: Record<ToolStatusName, string> = {
  "not-installed": "border-border bg-muted text-muted-foreground",
  inactive: "border-border bg-muted text-muted-foreground",
  active: "border-primary/30 bg-primary/10 text-primary",
  drift: "border-amber-600/30 bg-amber-600/10 text-amber-600 dark:text-amber-500",
  conflict: "border-destructive/30 bg-destructive/10 text-destructive",
  "recovery-required": "border-destructive/30 bg-destructive/10 text-destructive",
  unavailable: "border-border bg-muted text-muted-foreground",
};

export function StatusBadge({ status }: { status: ToolStatusName | null }) {
  const { t } = useTranslation();
  if (!status)
    return (
      <span className="inline-flex items-center rounded-lg border border-border bg-muted px-2 py-0.5 text-[13px] text-muted-foreground">
        {t("common.unknown")}
      </span>
    );
  return (
    <span
      className={cx(
        "inline-flex items-center rounded-lg border px-2 py-0.5 text-[13px] font-medium",
        STATUS_TONE[status],
      )}
    >
      {t(`status.${status}`)}
    </span>
  );
}
