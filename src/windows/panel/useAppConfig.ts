import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

export type AppConfig = {
  autostart: boolean;
  showCapsule: boolean;
  showTitleInMenuBar: boolean;
  islandOffsetX: number;
  islandOffsetY: number;
  /** 0=不透明，最高 50%；作用于悬浮岛、面板、快速新增全部 UI */
  capsuleTransparencyPercent: number;
  /** 全局快捷键，格式 "Ctrl+Alt+N" */
  quickAddShortcut: string;
  /** 面板显示模式："standard"=440px（对齐悬浮岛），"wide"=560px */
  displayMode: "standard" | "wide";
  /** 启动后后台预加载主面板数据 */
  panelBackgroundPreload: boolean;
  /** 主面板挂载后的自动刷新间隔（秒） */
  panelRefreshIntervalSecs: number;
};

const DEFAULTS: AppConfig = {
  autostart: false,
  showCapsule: true,
  showTitleInMenuBar: false,
  islandOffsetX: 0,
  islandOffsetY: 8,
  capsuleTransparencyPercent: 0,
  quickAddShortcut: "Ctrl+Alt+N",
  displayMode: "standard",
  panelBackgroundPreload: true,
  panelRefreshIntervalSecs: 60,
};

/** 系统级配置 — 与 Rust config.json / 托盘 / 悬浮岛联动 */
export function useAppConfig() {
  const [config, setConfig] = useState<AppConfig>(DEFAULTS);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void invoke<AppConfig>("get_app_config")
      .then((data) => {
        setConfig(data);
        setLoaded(true);
      })
      .catch(console.error);

    void listen<AppConfig>("app-config-changed", (event) => {
      setConfig(event.payload);
      setLoaded(true);
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => unlisten?.();
  }, []);

  const save = useCallback(async (patch: Partial<AppConfig>) => {
    setConfig((prev) => ({ ...prev, ...patch }));
    try {
      const saved = await invoke<AppConfig>("save_app_config", { patch });
      setConfig(saved);
      return saved;
    } catch (error) {
      const current = await invoke<AppConfig>("get_app_config");
      setConfig(current);
      throw error;
    }
  }, []);

  return { config, save, loaded };
}