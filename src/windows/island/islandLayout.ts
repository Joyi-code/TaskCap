import { invoke } from "@tauri-apps/api/core";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ensureIslandAlwaysOnTop } from "../../lib/windowZOrder";

export const ISLAND_SIZES = {
  collapsed: { width: 172, height: 30 },
  attention: { width: 340, height: 52 },
  expandedWidth: 440,
  expandedMinHeight: 92,
} as const;

export type IslandShellMode = "collapsed" | "attention" | "expanded";

export function resolveIslandSize(
  mode: IslandShellMode,
  expandedHeight?: number,
): { width: number; height: number } {
  if (mode === "expanded") {
    return {
      width: ISLAND_SIZES.expandedWidth,
      height: Math.max(ISLAND_SIZES.expandedMinHeight, Math.round(expandedHeight ?? ISLAND_SIZES.expandedMinHeight)),
    };
  }
  if (mode === "attention") {
    return { ...ISLAND_SIZES.attention };
  }
  return { ...ISLAND_SIZES.collapsed };
}

let resizeToken = 0;

/** 取消进行中的尺寸动画，供用户急点「关闭」时立刻切回收起态 */
export function cancelIslandResizeAnimation(): void {
  resizeToken += 1;
}

async function readLogicalOuterSize(): Promise<{ width: number; height: number }> {
  const win = getCurrentWindow();
  const [size, scale] = await Promise.all([win.outerSize(), win.scaleFactor()]);
  return {
    width: Math.round(size.width / scale),
    height: Math.round(size.height / scale),
  };
}

async function resizeIslandWindow(width: number, height: number): Promise<void> {
  await invoke("resize_island_window", { width, height });
}

/** 比较逻辑像素，避免高 DPI 下误判尺寸已到位 */
export async function windowMatchesTarget(width: number, height: number): Promise<boolean> {
  const logical = await readLogicalOuterSize();
  return logical.width === width && logical.height === height;
}

async function setLogicalSizeCentered(width: number, height: number): Promise<void> {
  const win = getCurrentWindow();
  await win.setResizable(true).catch(() => undefined);
  const [pos, outer, scale] = await Promise.all([
    win.outerPosition(),
    win.outerSize(),
    win.scaleFactor(),
  ]);
  const centerX = pos.x + outer.width / 2;
  const physicalW = Math.round(width * scale);
  const x = Math.round(centerX - physicalW / 2);
  await resizeIslandWindow(width, height);
  await win.setPosition(new PhysicalPosition(x, pos.y));
}

/** 窗口尺寸动画：保持顶边 Y、水平居中（对齐 macOS IslandPanelController） */
export async function animateIslandWindowSize(
  width: number,
  height: number,
  durationMs = 110,
): Promise<void> {
  if (await windowMatchesTarget(width, height)) {
    await ensureIslandAlwaysOnTop();
    return;
  }

  const token = ++resizeToken;
  const win = getCurrentWindow();
  await win.setResizable(true).catch(() => undefined);
  const [pos, scale] = await Promise.all([win.outerPosition(), win.scaleFactor()]);
  const start = await readLogicalOuterSize();
  const centerX = pos.x + Math.round(start.width * scale) / 2;
  const topY = pos.y;
  const steps = 10;
  const stepMs = durationMs / steps;

  for (let i = 1; i <= steps; i += 1) {
    if (token !== resizeToken) return;
    const t = i / steps;
    const eased = t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2;
    const w = Math.round(start.width + (width - start.width) * eased);
    const h = Math.round(start.height + (height - start.height) * eased);
    const physicalW = Math.round(w * scale);
    const x = Math.round(centerX - physicalW / 2);
    await resizeIslandWindow(w, h);
    await win.setPosition(new PhysicalPosition(x, topY));
    if (i < steps) {
      await new Promise((resolve) => setTimeout(resolve, stepMs));
    }
  }
  await ensureIslandAlwaysOnTop();
}

export async function applyIslandWindowSize(width: number, height: number): Promise<void> {
  if (await windowMatchesTarget(width, height)) {
    await ensureIslandAlwaysOnTop();
    return;
  }
  resizeToken += 1;
  await setLogicalSizeCentered(width, height);
  await ensureIslandAlwaysOnTop();
}
