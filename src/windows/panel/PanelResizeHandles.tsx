import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PointerEvent as ReactPointerEvent } from "react";

/** 与 @tauri-apps/api window 内部 ResizeDirection 一致（未导出，这里本地声明） */
type ResizeDirection =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

type HandleSpec = {
  key: string;
  direction: ResizeDirection;
};

/** 八方向手柄：四条边 + 四个角（尺寸全走 CSS） */
const HANDLES: HandleSpec[] = [
  { key: "n", direction: "North" },
  { key: "s", direction: "South" },
  { key: "w", direction: "West" },
  { key: "e", direction: "East" },
  { key: "nw", direction: "NorthWest" },
  { key: "ne", direction: "NorthEast" },
  { key: "sw", direction: "SouthWest" },
  { key: "se", direction: "SouthEast" },
];

/**
 * 面板窗口四周 + 四角的拖拽缩放层。
 * 窗口无边框且透明，WebView 覆盖整个客户区截走了边缘输入，
 * 因此用透明的边缘热区 + startResizeDragging 交还系统原生缩放。
 */
export function PanelResizeHandles() {
  const handlePointerDown = (
    event: ReactPointerEvent<HTMLDivElement>,
    direction: ResizeDirection,
  ) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    // startResizeDragging 会接管后续指针，交由系统完成缩放循环
    void getCurrentWindow().startResizeDragging(direction).catch(console.error);
  };

  return (
    <div className="panel-resize-layer" aria-hidden="true">
      {HANDLES.map((handle) => (
        <div
          key={handle.key}
          className={`panel-resize-handle panel-resize-${handle.key}`}
          onPointerDown={(event) => handlePointerDown(event, handle.direction)}
        />
      ))}
    </div>
  );
}
