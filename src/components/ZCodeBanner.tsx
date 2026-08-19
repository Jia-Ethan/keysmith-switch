import { useTranslation } from "react-i18next";
import { zcodeUnavailableReason } from "../lib/zcode";
import type { ToolInfo } from "../types";
import { IconAlert } from "./icons";

export function ZCodeBanner({ tool }: { tool: Pick<ToolInfo, "id" | "available" | "unavailableReason"> }) {
  const { t } = useTranslation();
  const reason = zcodeUnavailableReason(tool);
  if (!reason) return null;
  return (
    <div
      data-testid="zcode-unavailable"
      className="flex items-start gap-2 rounded-lg border border-amber-600/40 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-700 dark:text-amber-400"
      role="status"
    >
      <IconAlert size={14} className="mt-px shrink-0" />
      <div className="min-w-0">
        <p className="font-medium">{t("tool.unavailable")}</p>
        <p className="mt-0.5 break-words">{reason}</p>
        <p className="mt-0.5 opacity-80">{t("tool.zcodeNoInstall")}</p>
      </div>
    </div>
  );
}
