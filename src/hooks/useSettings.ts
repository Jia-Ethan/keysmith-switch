import { useCallback, useEffect, useState } from "react";
import { getSettings, updateSettings } from "../api";
import { applyLanguage } from "../i18n";
import type { Settings, SettingsPatch } from "../types";
import { DEFAULT_SETTINGS } from "../types";

export function useSettings() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [error, setError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);

  const reload = useCallback(async () => {
    try {
      const next = await getSettings();
      setSettings({ ...DEFAULT_SETTINGS, ...next });
      applyLanguage(next.language ?? DEFAULT_SETTINGS.language);
      setError(null);
    } catch (err) {
      setSettings(DEFAULT_SETTINGS);
      applyLanguage(DEFAULT_SETTINGS.language);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setReady(true);
    }
  }, []);

  const save = useCallback(async (patch: SettingsPatch) => {
    const next = await updateSettings(patch);
    setSettings({ ...DEFAULT_SETTINGS, ...next });
    if (next.language) applyLanguage(next.language);
    setError(null);
    return next;
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  return { settings, error, ready, reload, save, setSettings };
}
