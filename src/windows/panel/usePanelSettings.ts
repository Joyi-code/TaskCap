import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "taskcap.panel.settings";
const LEGACY_STORAGE_KEY = "taskisland.panel.settings";

export type PanelSettings = {
  darkGlassMode: boolean;
  defaultFocusMinutes: number;
  waterReminderEnabled: boolean;
  waterReminderMinutes: number;
  sittingReminderEnabled: boolean;
  sittingReminderMinutes: number;
};

const DEFAULTS: PanelSettings = {
  darkGlassMode: false,
  defaultFocusMinutes: 25,
  waterReminderEnabled: true,
  waterReminderMinutes: 45,
  sittingReminderEnabled: true,
  sittingReminderMinutes: 60,
};

export function readPanelSettings(): PanelSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULTS, ...JSON.parse(raw) };
    // 迁移旧 key
    const legacy = localStorage.getItem(LEGACY_STORAGE_KEY);
    if (legacy) {
      localStorage.setItem(STORAGE_KEY, legacy);
      localStorage.removeItem(LEGACY_STORAGE_KEY);
      return { ...DEFAULTS, ...JSON.parse(legacy) };
    }
    return DEFAULTS;
  } catch {
    return DEFAULTS;
  }
}

export function usePanelSettings() {
  const [settings, setSettings] = useState<PanelSettings>(readPanelSettings);

  const save = useCallback((patch: Partial<PanelSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...patch };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      return next;
    });
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("panel-dark", settings.darkGlassMode);
  }, [settings.darkGlassMode]);

  return { settings, save };
}