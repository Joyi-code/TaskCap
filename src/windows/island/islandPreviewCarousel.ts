import { useCallback, useEffect, useState } from "react";

/** 展开态固定高度：单行任务 + 右侧置顶按钮 */
export const ISLAND_EXPANDED_FIXED_HEIGHT = 92;

const AUTO_ADVANCE_MS = 4500;

export function usePreviewCarousel(
  taskCount: number,
  active: boolean,
  resetKey: string,
) {
  const [index, setIndex] = useState(0);
  const [autoPaused, setAutoPaused] = useState(false);

  useEffect(() => {
    setIndex(0);
    setAutoPaused(false);
  }, [resetKey]);

  useEffect(() => {
    if (taskCount <= 0) {
      setIndex(0);
      return;
    }
    setIndex((current) => Math.min(current, taskCount - 1));
  }, [taskCount]);

  const step = useCallback(
    (delta: number) => {
      if (taskCount <= 1) return;
      setAutoPaused(true);
      setIndex((current) => (current + delta + taskCount) % taskCount);
    },
    [taskCount],
  );

  const onWheel = useCallback(
    (deltaY: number) => {
      if (!active || taskCount <= 1) return false;
      step(deltaY > 0 ? 1 : -1);
      return true;
    },
    [active, step, taskCount],
  );

  useEffect(() => {
    if (!active || autoPaused || taskCount <= 1) return;
    const timer = setInterval(() => {
      setIndex((current) => (current + 1) % taskCount);
    }, AUTO_ADVANCE_MS);
    return () => clearInterval(timer);
  }, [active, autoPaused, taskCount]);

  return { index, step, onWheel, taskCount };
}