import { useTranslation } from "react-i18next";
import type { ToolStatusName } from "../types";
import { cx } from "./ui";

const tone: Record<ToolStatusName, string> = {
  "not-installed": "bg-ink-700 text-ink-100",
  inactive: "bg-ink-700 text-ink-100",
  active: "bg-teal-900/70 text-teal-200",
  drift: "bg-amber-900/70 text-amber-200",
  conflict: "bg-rose-900/70 text-rose-200",
  "recovery-required": "bg-rose-900/70 text-rose-200",
  unavailable: "bg-ink-700 text-ink-200",
};

export function StatusBadge({ status }: { status: ToolStatusName | null }) {
  const { t } = useTranslation();
  if (!status) return <span className="rounded bg-ink-700 px-1.5 py-0.5 text-[11px]">{t("common.unknown")}</span>;
  return (
    <span className={cx("rounded px-1.5 py-0.5 text-[11px] font-medium", tone[status])}>
      {t(`status.${status}`)}
    </span>
  );
}
