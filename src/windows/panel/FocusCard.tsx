import { invoke } from "@tauri-apps/api/core";
import { Pause, Play, Square, Timer, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { formatFocusTime, TaskDetail } from "./panelTypes";
import { readPanelSettings } from "./usePanelSettings";

type Props = {
  task: TaskDetail | null;
  menuTitle: string;
  onChanged: () => void;
};

type FocusAnchor = {
  taskId: string;
  seconds: number;
  atMs: number;
};

/** 当前任务 / 专注卡片 —— 紧凑横向布局：左图标 + 标题/状态 + 右侧操作 */
export function FocusCard({ task, menuTitle, onChanged }: Props) {
  const anchorRef = useRef<FocusAnchor | null>(null);
  const [, setTick] = useState(0);

  // 仅在任务切换或专注开始/暂停时重设锚点，避免 snapshot 刷新把计时清零
  useEffect(() => {
    if (!task) {
      anchorRef.current = null;
      return;
    }
    anchorRef.current = {
      taskId: task.id,
      seconds: task.focusSeconds,
      atMs: Date.now(),
    };
  }, [task?.id, task?.isFocusRunning]);

  useEffect(() => {
    if (!task?.isFocusRunning) return;
    const timer = setInterval(() => setTick((value) => value + 1), 1000);
    return () => clearInterval(timer);
  }, [task?.isFocusRunning, task?.id]);

  const title = task?.title ?? menuTitle;
  const seconds = resolveFocusSeconds(task, anchorRef.current);
  const targetMinutes = task?.estimatedMinutes ?? readPanelSettings().defaultFocusMinutes;
  const remaining = Math.max(0, targetMinutes * 60 - seconds);

  const isPaused = !!task && !task.isFocusRunning && !task.isCompleted && seconds > 0;
  const sub = !task
    ? "暂无当前任务"
    : task.isFocusRunning
      ? `专注中，剩余 ${formatFocusTime(remaining)}`
      : isPaused
        ? `已暂停，剩余 ${formatFocusTime(remaining)}`
        : `一轮 ${targetMinutes} 分 · 点击开始`;

  async function run(action: "start" | "pause" | "stop" | "close") {
    if (!task) return;
    if (action === "start") await invoke("start_focus", { id: task.id });
    if (action === "pause") await invoke("pause_focus", { id: task.id });
    if (action === "stop") await invoke("stop_focus", { id: task.id });
    if (action === "close") await invoke("close_focus", { id: task.id });
    onChanged();
  }

  return (
    <div className="panel-focus-card">
      <div className="panel-focus-icon">
        <Timer size={16} />
      </div>
      <div className="panel-focus-body">
        <div className="panel-focus-line1">
          <span className="panel-focus-title">{title}</span>
        </div>
        <div className="panel-focus-sub">{sub}</div>
      </div>
      {task && !task.isCompleted ? (
        <div className="panel-focus-actions">
          <button
            type="button"
            className="panel-focus-btn"
            aria-label="关闭专注任务"
            onClick={() => void run("close")}
          >
            <X size={13} />
          </button>
          <button
            type="button"
            className="panel-focus-btn"
            aria-label={task.isFocusRunning ? "暂停" : isPaused ? "继续" : "开始专注"}
            onClick={() => void run(task.isFocusRunning ? "pause" : "start")}
          >
            {task.isFocusRunning ? <Pause size={13} /> : <Play size={13} />}
          </button>
          {task.isFocusRunning || isPaused ? (
            <button
              type="button"
              className="panel-focus-btn"
              aria-label="停止"
              onClick={() => void run("stop")}
            >
              <Square size={12} />
            </button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

function resolveFocusSeconds(task: TaskDetail | null, anchor: FocusAnchor | null): number {
  if (!task) return 0;
  if (!task.isFocusRunning) return task.focusSeconds;
  if (!anchor || anchor.taskId !== task.id) return task.focusSeconds;
  return anchor.seconds + (Date.now() - anchor.atMs) / 1000;
}
