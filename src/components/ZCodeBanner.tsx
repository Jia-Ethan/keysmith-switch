import { useTranslation } from "react-i18next";
import { zcodeUnavailableReason } from "../lib/zcode";
import type { ToolInfo } from "../types";

export function ZCodeBanner({ tool }: { tool: Pick<ToolInfo, "id" | "available" | "unavailableReason"> }) {
  const { t } = useTranslation();
  const reason = zcodeUnavailableReason(tool);
  if (!reason) return null;
  return (
    <div
      data-testid="zcode-unavailable"
      className="rounded border border-amber-800/50 bg-amber-950/40 px-3 py-2 text-[12px] text-amber-100"
      role="status"
    >
      <p className="font-medium">{t("tool.unavailable")}</p>
      <p className="mt-1">{reason}</p>
      <p className="mt-1 text-amber-200/80">{t("tool.zcodeNoInstall")}</p>
    </div>
  );
}
