import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import { clampTransparencyPercent, resolveAppGlassStyle } from "./appGlassStyle";
import type { AppConfig } from "../windows/panel/useAppConfig";

/** 订阅全局界面透明度（悬浮岛 + 面板 + 快速新增） */
export function useUiTransparency(prefersDark: boolean) {
  const [percent, setPercent] = useState(0);

  useEffect(() => {
    void invoke<AppConfig>("get_app_config")
      .then((config) =>
        setPercent(clampTransparencyPercent(config.capsuleTransparencyPercent ?? 0)),
      )
      .catch(console.error);

    let unlisten: (() => void) | undefined;
    void listen<AppConfig>("app-config-changed", (event) => {
      setPercent(
        clampTransparencyPercent(event.payload.capsuleTransparencyPercent ?? 0),
      );
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => unlisten?.();
  }, []);

  const glassStyle = useMemo(
    () => resolveAppGlassStyle(percent, prefersDark),
    [percent, prefersDark],
  );

  return {
    transparencyPercent: percent,
    glassStyle,
    isGlassActive: percent > 0,
  };
}