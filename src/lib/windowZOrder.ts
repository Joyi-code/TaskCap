import { getCurrentWindow } from "@tauri-apps/api/window";

/** 将当前窗口设为系统顶层（Windows HWND_TOPMOST） */
export async function setWindowAlwaysOnTop(enabled: boolean): Promise<void> {
  await getCurrentWindow().setAlwaysOnTop(enabled);
}

/** 悬浮岛应始终置顶；尺寸/位置变更后需重新声明，避免被其它窗口压住 */
export async function ensureIslandAlwaysOnTop(): Promise<void> {
  await setWindowAlwaysOnTop(true);
}