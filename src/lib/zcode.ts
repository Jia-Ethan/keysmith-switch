import type { ToolInfo } from "../types";

export const ZCODE_WINDOWS_REASON =
  "ZCode is only available on macOS. Windows builds do not ship or install ZCode.";

export function isWindowsUserAgent(ua: string): boolean {
  return /windows/i.test(ua);
}

export function isZcodeUnavailable(info: Pick<ToolInfo, "id" | "available">): boolean {
  return info.id === "zcode" && info.available === false;
}

export function zcodeUnavailableReason(
  info: Pick<ToolInfo, "id" | "available" | "unavailableReason">,
  userAgent = typeof navigator !== "undefined" ? navigator.userAgent : "",
): string | null {
  if (!isZcodeUnavailable(info)) return null;
  if (info.unavailableReason && info.unavailableReason.trim()) {
    return info.unavailableReason;
  }
  if (isWindowsUserAgent(userAgent)) return ZCODE_WINDOWS_REASON;
  return ZCODE_WINDOWS_REASON;
}
