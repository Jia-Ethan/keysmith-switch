export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function openExternal(url: string): Promise<void> {
  if (!url) return;
  try {
    if (isTauriRuntime()) {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(url);
      return;
    }
  } catch {
    // fall through to window.open
  }
  if (typeof window !== "undefined") {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}

export async function pickFiles(filters?: Array<{ name: string; extensions: string[] }>): Promise<string[]> {
  try {
    if (!isTauriRuntime()) return [];
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({ multiple: true, filters });
    if (!selected) return [];
    return Array.isArray(selected) ? selected : [selected];
  } catch {
    return [];
  }
}

export async function pickSavePath(defaultPath: string): Promise<string | null> {
  try {
    if (!isTauriRuntime()) return null;
    const { save } = await import("@tauri-apps/plugin-dialog");
    const selected = await save({ defaultPath });
    return typeof selected === "string" ? selected : null;
  } catch {
    return null;
  }
}

export async function pickDirectory(): Promise<string | null> {
  try {
    if (!isTauriRuntime()) return null;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({ directory: true, multiple: false });
    return typeof selected === "string" ? selected : null;
  } catch {
    return null;
  }
}
