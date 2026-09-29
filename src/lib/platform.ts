import { isTauriRuntime } from "./runtime";

export type Platform = "mac" | "windows" | "other";

export function detectPlatform(): Platform {
  if (typeof navigator === "undefined") return "other";
  const hint = `${(navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ?? ""} ${navigator.platform ?? ""} ${navigator.userAgent ?? ""}`;
  if (/mac/i.test(hint)) return "mac";
  if (/win/i.test(hint)) return "windows";
  return "other";
}

/**
 * Marks the document with the platform. On macOS the desktop build uses an
 * overlay title bar (see tauri.macos.conf.json), so the header must leave room
 * for the traffic lights; that only holds inside the Tauri window.
 */
export function applyPlatformAttributes(root: HTMLElement = document.documentElement): void {
  const platform = detectPlatform();
  root.dataset.platform = platform;
  if (platform === "mac" && isTauriRuntime()) root.dataset.titlebar = "overlay";
  else delete root.dataset.titlebar;
}
