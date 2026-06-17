import { getCurrentWindow } from "@tauri-apps/api/window";
import { PointerEvent as ReactPointerEvent } from "react";

const DRAG_THRESHOLD_PX = 4;

/** 记录按下位置，用于区分拖动与点击（对齐 Mac IslandPanelController） */
export function createPointerDragTracker() {
  let start: { x: number; y: number } | null = null;
  let didDrag = false;
  let lastInteractionWasDrag = false;
  let nativeDragStarted = false;

  return {
    onPointerDown(event: ReactPointerEvent) {
      if (event.button !== 0) return false;
      start = { x: event.clientX, y: event.clientY };
      didDrag = false;
      lastInteractionWasDrag = false;
      nativeDragStarted = false;
      return true;
    },
    onPointerMove(event: ReactPointerEvent) {
      if (!start) return;
      const dx = event.clientX - start.x;
      const dy = event.clientY - start.y;
      if (Math.hypot(dx, dy) > DRAG_THRESHOLD_PX) {
        didDrag = true;
        if (!nativeDragStarted) {
          nativeDragStarted = true;
          void getCurrentWindow().startDragging();
        }
      }
    },
    onPointerUp() {
      lastInteractionWasDrag = didDrag;
      start = null;
      didDrag = false;
      nativeDragStarted = false;
      return lastInteractionWasDrag;
    },
    wasClick() {
      return !lastInteractionWasDrag;
    },
    resetClickGuard() {
      lastInteractionWasDrag = false;
    },
  };
}
