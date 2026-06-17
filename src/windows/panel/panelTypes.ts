export type TaskCounts = {
  high: number;
  medium: number;
  low: number;
  total: number;
};

export type TaskDetail = {
  id: string;
  title: string;
  notes: string;
  priority: number;
  isCompleted: boolean;
  isMarkedComplete: boolean;
  isCurrent: boolean;
  isInTodayQueue: boolean;
  dueAt: string | null;
  reminderAt: string | null;
  tags: string[];
  projectName: string | null;
  estimatedMinutes: number | null;
  repeatRule: string | null;
  focusSeconds: number;
  isFocusRunning: boolean;
  subtaskDone: number;
  subtaskTotal: number;
  completedAt: string | null;
};

export type PanelReview = {
  completedToday: number;
  postponedToday: number;
  tomorrowCount: number;
  focusMinutes: number;
};

export type PanelSnapshot = {
  menuBarTitle: string;
  counts: TaskCounts;
  todayCount: number;
  incomplete: TaskDetail[];
  completed: TaskDetail[];
  suggested: TaskDetail[];
  allTags: string[];
  allTagSuggestions: string[];
  allProjects: string[];
  review: PanelReview;
  focusTask: TaskDetail | null;
};

export type SearchTasksResult = {
  incomplete: TaskDetail[];
  completed: TaskDetail[];
};

export const PRIORITY_LABEL: Record<number, string> = {
  0: "高",
  1: "中",
  2: "低",
};

export function formatFocusTime(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function formatDueLabel(iso: string | null): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
