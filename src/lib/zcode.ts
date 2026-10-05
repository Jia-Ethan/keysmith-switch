import type { ToolInfo } from "../types";

export const ZCODE_UNAVAILABLE_REASON = "ZCode is unavailable on this system.";

export function isZcodeUnavailable(info: Pick<ToolInfo, "id" | "available">): boolean {
  return info.id === "zcode" && info.available === false;
}

export function zcodeUnavailableReason(
  info: Pick<ToolInfo, "id" | "available" | "unavailableReason">,
): string | null {
  if (!isZcodeUnavailable(info)) return null;
  if (info.unavailableReason && info.unavailableReason.trim()) {
    return info.unavailableReason;
  }
  return ZCODE_UNAVAILABLE_REASON;
}
