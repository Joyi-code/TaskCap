import type { CSSProperties } from "react";

/** 界面透明度滑块上限（0=不透明，50=最透） */
export const MAX_UI_TRANSPARENCY_PERCENT = 50;

/** 0=不透明，上限见 MAX_UI_TRANSPARENCY_PERCENT（配置键 capsuleTransparencyPercent） */
export function clampTransparencyPercent(value: number): number {
  return Math.min(
    MAX_UI_TRANSPARENCY_PERCENT,
    Math.max(0, Math.round(value)),
  );
}

const DEFAULT_PROGRESS = 0.72;

function uiOpacityProgress(percent: number): number {
  return 1 - clampTransparencyPercent(percent) / 100;
}

/** 高透明度时整体淡出（对齐 macOS capsuleVisibilityScale） */
function uiVisibilityScale(progress: number): number {
  if (progress >= DEFAULT_PROGRESS) return 1;
  return progress / DEFAULT_PROGRESS;
}

/** 低透明度设置时加厚底色；0% 时完全不透视桌面（对齐 macOS capsuleDensityBoost） */
function uiDensityBoost(progress: number): number {
  if (progress <= DEFAULT_PROGRESS) return 0;
  return Math.min((progress - DEFAULT_PROGRESS) / (1 - DEFAULT_PROGRESS), 1);
}

function boostedAlpha(base: number, boost: number, boostFactor: number): number {
  return Math.min(base * (1 + boost * boostFactor) + boost * (1 - base), 1);
}

/**
 * 将界面透明度百分比映射为任务岛玻璃样式（悬浮岛 / 面板 / 快速新增共用）。
 * 0%：实心背景 + 无 backdrop 模糊；50%：允许的最透状态。
 */
export function resolveAppGlassStyle(
  transparencyPercent: number,
  prefersDark = false,
  tealTheme = false,
): CSSProperties {
  const progress = uiOpacityProgress(transparencyPercent);
  const visibilityScale = uiVisibilityScale(progress);
  const densityBoost = uiDensityBoost(progress);

  const baseTop = prefersDark ? 0.82 : 0.78;
  const baseBottom = prefersDark ? 0.68 : 0.52;
  const boostFactor = prefersDark ? 0.3 : 0.45;

  // visibilityScale 乘进 alpha，不用 CSS opacity，避免 WebView2 刷新任务栏图标导致闪烁
  const topAlpha = boostedAlpha(baseTop, densityBoost, boostFactor) * visibilityScale;
  const bottomAlpha = boostedAlpha(baseBottom, densityBoost, boostFactor) * visibilityScale;

  const blurPx = densityBoost >= 0.95 ? 0 : Math.round(24 * (1 - (1 - progress) * 0.85));
  const backdropFilter = blurPx > 0 ? `blur(${blurPx}px) saturate(1.55)` : "none";

  const topColor = prefersDark
    ? `rgba(36, 38, 52, ${topAlpha})`
    : tealTheme
      ? `rgba(165, 226, 211, ${topAlpha})`
      : `rgba(255, 255, 255, ${topAlpha})`;
  const bottomColor = prefersDark
    ? `rgba(24, 26, 36, ${bottomAlpha})`
    : tealTheme
      ? `rgba(98, 182, 163, ${bottomAlpha})`
      : `rgba(255, 255, 255, ${bottomAlpha})`;

  const borderAlpha = (prefersDark
    ? Math.min(0.14 + densityBoost * 0.86, 1)
    : Math.min(0.55 + densityBoost * 0.45, 1)) * visibilityScale;

  return {
    background: `linear-gradient(180deg, ${topColor} 0%, ${bottomColor} 100%)`,
    backdropFilter,
    WebkitBackdropFilter: backdropFilter,
    borderColor: `rgba(255, 255, 255, ${borderAlpha})`,
  };
}