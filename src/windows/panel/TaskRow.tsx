import { invoke } from "@tauri-apps/api/core";
import { CalendarDays, Check, ChevronDown, ChevronUp, Play, Sunrise, Sun, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { formatDueLabel, PRIORITY_LABEL, TaskDetail } from "./panelTypes";
import { PanelDateField } from "./PanelDateField";

type Props = {
  task: TaskDetail;
  busy: boolean;
  expanded: boolean;
  onToggleExpand: () => void;
  onChanged: () => void;
  /** 删除任务前回调（用于弹出撤销提示） */
  onCompleted?: (task: TaskDetail) => void;
};

/** 优先级 → 彩标样式类（0 高 / 1 中 / 2 低） */
const PRIORITY_CLASS: Record<number, string> = { 0: "high", 1: "mid", 2: "low" };
const pad = (n: number) => String(n).padStart(2, "0");

/** 元信息行：用图标标识今天、截止、提醒和专注状态 */
function getTomorrow6pm(): string {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  d.setHours(18, 0, 0, 0);
  return d.toISOString();
}

function isTomorrow(iso: string | null): boolean {
  if (!iso) return false;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return false;
  const tomorrow = new Date();
  tomorrow.setDate(tomorrow.getDate() + 1);
  return (
    d.getFullYear() === tomorrow.getFullYear() &&
    d.getMonth() === tomorrow.getMonth() &&
    d.getDate() === tomorrow.getDate()
  );
}

function TaskMeta({
  task,
  busy,
  onFocusToggle,
  onTodayToggle,
  onSetTomorrow,
}: {
  task: TaskDetail;
  busy: boolean;
  onFocusToggle: () => void;
  onTodayToggle: () => void;
  onSetTomorrow: (isActive: boolean) => void;
}) {
  const hasMeta =
    (task.isCompleted && task.completedAt) ||
    !task.isCompleted ||
    task.isInTodayQueue ||
    task.isCurrent ||
    task.dueAt ||
    task.reminderAt ||
    task.tags.length > 0;
  if (!hasMeta) return null;

  return (
    <span className="panel-task-meta">
      {!task.isCompleted && task.isFocusRunning ? (
        <button
          type="button"
          className="panel-task-meta-chip focus is-running"
          disabled={busy}
          onClick={(event) => {
            event.stopPropagation();
            onFocusToggle();
          }}
        >
          专注中
        </button>
      ) : null}
      {!task.isCompleted && !task.isFocusRunning ? (
        <button
          type="button"
          className="panel-task-meta-chip focus"
          disabled={busy}
          onClick={(event) => {
            event.stopPropagation();
            onFocusToggle();
          }}
        >
          <Play size={11} />
          专注
        </button>
      ) : null}
      {task.isCompleted && task.completedAt ? (
        <span className="panel-task-meta-chip">{formatDueLabel(task.completedAt)}</span>
      ) : null}
      {!task.isCompleted ? (
        <button
          type="button"
          className={`panel-task-meta-chip today${task.isInTodayQueue ? " is-active" : ""}`}
          disabled={busy}
          onClick={(event) => {
            event.stopPropagation();
            onTodayToggle();
          }}
        >
          <Sun size={11} />
          今天
        </button>
      ) : null}
      {!task.isCompleted ? (
        <button
          type="button"
          className={`panel-task-meta-chip tomorrow${isTomorrow(task.dueAt) ? " is-active" : ""}`}
          disabled={busy}
          title={isTomorrow(task.dueAt) ? "取消明天" : "截止时间设为明天 18:00"}
          onClick={(event) => {
            event.stopPropagation();
            onSetTomorrow(isTomorrow(task.dueAt));
          }}
        >
          <Sunrise size={11} />
          明天
        </button>
      ) : null}
      {task.dueAt ? (() => {
        const isOverdue = new Date(task.dueAt) < new Date();
        return (
          <span className={`panel-task-meta-chip due${isOverdue ? " is-overdue" : ""}`}>
            <CalendarDays size={11} />
            {isOverdue ? "已超时" : formatCompactDate(task.dueAt)}
          </span>
        );
      })() : null}
      {task.tags.length ? (
        <span className="panel-task-meta-chip tags">{task.tags.map((t) => `#${t}`).join(" ")}</span>
      ) : null}
    </span>
  );
}

/** RFC3339(UTC) → 本地日期时间字符串 */
function toLocalInput(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** 本地日期时间字符串 → RFC3339(UTC) */
function fromLocalInput(local: string): string | null {
  if (!local) return null;
  const d = new Date(local);
  if (Number.isNaN(d.getTime())) return null;
  return d.toISOString();
}

function formatCompactDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return formatDueLabel(iso);
  return `${d.getMonth() + 1}/${d.getDate()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/**
 * 任务行 + 展开详情。
 * 极简交互：全部字段改完即存（失焦/选完自动保存），无保存按钮、无勾选框。
 * 展开顺序：标题 / 专注 / 截止 / 提醒 / 优先级 / 标签 / 备注。
 * 详情区作为任务行（flex-wrap）的整宽子项独占一行，避免被右侧按钮列挤压留白。
 */
export function TaskRow({ task, busy, expanded, onToggleExpand, onChanged, onCompleted }: Props) {
  const [titleDraft, setTitleDraft] = useState(task.title);
  const [notesDraft, setNotesDraft] = useState(task.notes);
  const [dueDraft, setDueDraft] = useState(toLocalInput(task.dueAt));
  const [reminderDraft, setReminderDraft] = useState(toLocalInput(task.reminderAt));
  const [estimatedDraft, setEstimatedDraft] = useState(task.estimatedMinutes?.toString() ?? "");
  const [tagsDraft, setTagsDraft] = useState(task.tags.join(" "));
  const [activeDateField, setActiveDateField] = useState<"due" | "reminder" | null>(null);
  const [confirmPending, setConfirmPending] = useState<{ dueAt: string | null; reminderAt: string | null } | null>(null);

  useEffect(() => {
    setTitleDraft(task.title);
    setNotesDraft(task.notes);
    setDueDraft(toLocalInput(task.dueAt));
    setReminderDraft(toLocalInput(task.reminderAt));
    setEstimatedDraft(task.estimatedMinutes?.toString() ?? "");
    setTagsDraft(task.tags.join(" "));
  }, [task.id, task.title, task.notes, task.dueAt, task.reminderAt, task.estimatedMinutes, task.tags]);

  async function run(action: string, payload: Record<string, unknown> = {}) {
    await invoke(action, { id: task.id, ...payload });
    onChanged();
  }

  async function complete() {
    await invoke("complete_task", { id: task.id });
    onChanged();
  }

  async function toggleFocus() {
    await invoke(task.isFocusRunning ? "pause_focus" : "start_focus", { id: task.id });
    onChanged();
  }

  async function toggleToday() {
    await invoke("toggle_today_queue", { id: task.id });
    if (!task.isInTodayQueue && task.dueAt) {
      await invoke("set_task_due_reminder", {
        id: task.id,
        dueAt: null,
        reminderAt: fromLocalInput(reminderDraft),
      });
    }
    onChanged();
  }

  async function setToTomorrow(isActive: boolean) {
    await invoke("set_task_due_reminder", {
      id: task.id,
      dueAt: isActive ? null : getTomorrow6pm(),
      reminderAt: fromLocalInput(reminderDraft),
    });
    if (!isActive && task.isInTodayQueue) {
      await invoke("toggle_today_queue", { id: task.id });
    }
    onChanged();
  }

  // 截止 + 提醒共用一个后端命令，两者一起提交当前值
  function saveDueReminder(dueLocal: string, reminderLocal: string) {
    const dueAt = fromLocalInput(dueLocal);
    const reminderAt = fromLocalInput(reminderLocal);
    if (dueAt && reminderAt && new Date(reminderAt).getTime() > new Date(dueAt).getTime()) {
      // window.confirm 在 Tauri WebView 中无效，改用内联确认
      setConfirmPending({ dueAt, reminderAt });
      return;
    }
    void run("set_task_due_reminder", { dueAt, reminderAt });
  }

  function confirmSaveDueReminder() {
    if (!confirmPending) return;
    void run("set_task_due_reminder", confirmPending);
    setConfirmPending(null);
  }

  function cancelSaveDueReminder() {
    setDueDraft(toLocalInput(task.dueAt));
    setReminderDraft(toLocalInput(task.reminderAt));
    setConfirmPending(null);
  }

  function saveTags() {
    const tags = tagsDraft
      .split(/[\s,，#]+/)
      .map((t) => t.trim())
      .filter(Boolean);
    if (task.tags.join(" ") !== tags.join(" ")) {
      void run("set_task_tags", { tags });
    }
  }

  function saveEstimatedMinutes() {
    const minutes = estimatedDraft.trim() ? Number(estimatedDraft) : null;
    const normalized = Number.isFinite(minutes) && minutes && minutes > 0 ? Math.round(minutes) : null;
    if ((task.estimatedMinutes ?? null) !== normalized) {
      void run("set_task_estimated_minutes", { minutes: normalized });
    }
  }

  return (
    <li className={`panel-task-row${task.isCurrent ? " is-current" : ""}`}>
      <button
        type="button"
        className={`panel-task-check${task.isCompleted ? " is-done" : ""}`}
        aria-label={task.isCompleted ? "取消归档" : "归档"}
        title={task.isCompleted ? "取消归档" : "归档"}
        disabled={busy}
        onClick={() => (task.isCompleted ? void run("reopen_task") : void complete())}
      >
        {task.isCompleted ? <Check size={14} /> : null}
      </button>

      <div className="panel-task-body">
        <div className="panel-task-top-row">
          <div className="panel-task-main-col">
            <button
              type="button"
              className="panel-task-main"
              disabled={busy}
              onClick={() => onToggleExpand()}
            >
              <span className="panel-task-title">{task.title}</span>
            </button>
          </div>

          <span className={`panel-task-prio ${PRIORITY_CLASS[task.priority] ?? "mid"}`}>
            {PRIORITY_LABEL[task.priority] ?? "中"}
          </span>

          {!task.isCompleted ? (
            <button
              type="button"
              className="icon-btn panel-task-delete-btn"
              aria-label="删除"
              title="删除"
              disabled={busy}
              onClick={() => onCompleted?.(task)}
            >
              <Trash2 size={13} />
            </button>
          ) : null}

          <button
            type="button"
            className="icon-btn panel-expand-btn"
            aria-label={expanded ? "收起" : "展开"}
            onClick={() => onToggleExpand()}
          >
            {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>
        </div>

        <div className="panel-task-meta-row">
          <TaskMeta
            task={task}
            busy={busy}
            onFocusToggle={() => void toggleFocus()}
            onTodayToggle={() => void toggleToday()}
            onSetTomorrow={(active) => void setToTomorrow(active)}
          />
        </div>

        {expanded ? (
        <div className="panel-task-detail">
          <div className="panel-detail-field">
            <span className="panel-detail-field-label">任务</span>
            <input
              className="panel-detail-input"
              value={titleDraft}
              onChange={(e) => setTitleDraft(e.target.value)}
              onBlur={() => {
                if (titleDraft.trim() && titleDraft !== task.title) {
                  void run("update_task_title", { title: titleDraft.trim() });
                }
              }}
            />
          </div>


          <div className="panel-detail-field">
            <span className="panel-detail-field-label">截止</span>
            <PanelDateField
              value={dueDraft}
              open={activeDateField === "due"}
              onOpenChange={(nextOpen) => setActiveDateField(nextOpen ? "due" : null)}
              onChange={(next) => {
                setDueDraft(next);
                saveDueReminder(next, reminderDraft);
              }}
            />
          </div>

          <div className="panel-detail-field">
            <span className="panel-detail-field-label">提醒</span>
            <PanelDateField
              value={reminderDraft}
              open={activeDateField === "reminder"}
              onOpenChange={(nextOpen) => setActiveDateField(nextOpen ? "reminder" : null)}
              onChange={(next) => {
                setReminderDraft(next);
                saveDueReminder(dueDraft, next);
              }}
            />
          </div>

          {confirmPending ? (
            <div className="panel-reminder-confirm">
              <span className="panel-reminder-confirm-text">提醒时间晚于截止时间，确认这样设置吗？</span>
              <div className="panel-reminder-confirm-actions">
                <button type="button" className="panel-chip-btn is-warn" onClick={confirmSaveDueReminder}>确认</button>
                <button type="button" className="panel-chip-btn" onClick={cancelSaveDueReminder}>取消</button>
              </div>
            </div>
          ) : null}

          <div className="panel-detail-field">
            <span className="panel-detail-field-label panel-detail-field-label-wide">任务完成预计时长</span>
            <div className="panel-estimate-control">
              <input
                className="panel-detail-input"
                type="number"
                min={1}
                step={5}
                placeholder="25"
                value={estimatedDraft}
                onChange={(e) => setEstimatedDraft(e.target.value)}
                onBlur={saveEstimatedMinutes}
              />
              <span>分钟</span>
            </div>
          </div>

          {/* 优先级 */}
          <div className="panel-detail-actions">
            {([0, 1, 2] as const).map((p) => (
              <button
                key={p}
                type="button"
                className={`panel-chip-btn${task.priority === p ? " is-active" : ""}`}
                onClick={() => void run("set_task_priority", { priority: p })}
              >
                {PRIORITY_LABEL[p]}
              </button>
            ))}
          </div>

          {/* 标签 */}
          <div className="panel-detail-field">
            <span className="panel-detail-field-label">标签</span>
            <input
              className="panel-detail-input"
              placeholder="空格分隔"
              value={tagsDraft}
              onChange={(e) => setTagsDraft(e.target.value)}
              onBlur={saveTags}
            />
          </div>

          {/* 备注：label 顶部对齐多行输入框 */}
          <div className="panel-detail-field panel-detail-field-top">
            <span className="panel-detail-field-label">备注</span>
            <textarea
              className="panel-detail-notes"
              placeholder="补充说明"
              value={notesDraft}
              onChange={(e) => setNotesDraft(e.target.value)}
              onBlur={() => {
                if (notesDraft !== task.notes) {
                  void run("update_task_notes", { notes: notesDraft });
                }
              }}
            />
          </div>
        </div>
        ) : null}
      </div>
    </li>
  );
}
