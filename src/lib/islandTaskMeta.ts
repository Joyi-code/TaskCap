/** 悬浮岛任务摘要（与 Rust TaskSummary 对齐） */
export type IslandTaskSummary = {
  id: string;
  title: string;
  priority: number;
  isCompleted: boolean;
  isMarkedComplete: boolean;
  isCurrent: boolean;
  isInTodayQueue: boolean;
  dueAt?: string | null;
  reminderAt?: string | null;
  estimatedMinutes?: number | null;
  focusRemainingSeconds?: number | null;
  isFocusRunning?: boolean;
};

function sameLocalDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

function formatHm(date: Date): string {
  const h = date.getHours();
  const m = date.getMinutes();
  return `${h}:${String(m).padStart(2, "0")}`;
}

/** 对齐 macOS islandDateText，并保留「今天/明天」前缀 */
export function formatIslandDateText(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;

  const now = new Date();
  const tomorrow = new Date(now);
  tomorrow.setDate(tomorrow.getDate() + 1);

  const hm = formatHm(date);
  if (sameLocalDay(date, now)) return `今天 ${hm}`;
  if (sameLocalDay(date, tomorrow)) return `明天 ${hm}`;
  return `${date.getMonth() + 1}/${date.getDate()} ${hm}`;
}

function formatFocusDuration(seconds: number): string {
  const total = Math.max(Math.round(seconds), 0);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
  }
  return `${minutes}:${String(secs).padStart(2, "0")}`;
}

/** 展开预览行右侧的短标签：带「截止/提醒/专注」等前缀 */
export function formatIslandTaskMeta(task: IslandTaskSummary): string | null {
  if (task.isFocusRunning && task.focusRemainingSeconds != null) {
    return `专注剩 ${formatFocusDuration(task.focusRemainingSeconds)}`;
  }

  if (task.dueAt) {
    const due = new Date(task.dueAt);
    if (!Number.isNaN(due.getTime()) && due < new Date()) {
      return `已超时`;
    }
    return `截止 ${formatIslandDateText(task.dueAt)}`;
  }

  if (task.reminderAt) {
    return `提醒 ${formatIslandDateText(task.reminderAt)}`;
  }

  if (task.estimatedMinutes != null && task.estimatedMinutes > 0) {
    return `预估 ${task.estimatedMinutes}m`;
  }

  if (task.focusRemainingSeconds != null && task.focusRemainingSeconds > 0) {
    return `专注 ${formatFocusDuration(task.focusRemainingSeconds)}`;
  }

  return null;
}
