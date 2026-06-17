import { invoke } from "@tauri-apps/api/core";
import { ArrowUp, Search } from "lucide-react";
import { useMemo, useRef, useState } from "react";
import { FocusCard } from "./FocusCard";
import { PanelSnapshot, TaskDetail } from "./panelTypes";
import { PanelViewId, PANEL_VIEWS } from "./panelViews";
import { QuickAddInput } from "./QuickAddInput";
import { TaskRow } from "./TaskRow";

function TagFilterChips({
  tags,
  selected,
  onSelect,
}: {
  tags: string[];
  selected: string | null;
  onSelect: (tag: string | null) => void;
}) {
  return (
    <div className="panel-filter-chips">
      <button
        type="button"
        className={`panel-chip-btn${selected === null ? " is-active" : ""}`}
        onClick={() => onSelect(null)}
      >
        全部
      </button>
      {tags.map((tag) => (
        <button
          key={tag}
          type="button"
          className={`panel-chip-btn${selected === tag ? " is-active" : ""}`}
          onClick={() => onSelect(tag)}
        >
          #{tag}
        </button>
      ))}
    </div>
  );
}

type Props = {
  snapshot: PanelSnapshot;
  searchQuery: string;
  onSearchChange: (value: string) => void;
  searchResults: { incomplete: TaskDetail[]; completed: TaskDetail[] } | null;
  viewMode: PanelViewId;
  onViewModeChange: (mode: PanelViewId) => void;
  busyId: string | null;
  onRefresh: () => void;
};

function groupByTag(tasks: TaskDetail[]): Map<string, TaskDetail[]> {
  const map = new Map<string, TaskDetail[]>();
  for (const task of tasks) {
    if (task.tags.length === 0) {
      const list = map.get("未分类") ?? [];
      list.push(task);
      map.set("未分类", list);
      continue;
    }
    for (const tag of task.tags) {
      const list = map.get(tag) ?? [];
      list.push(task);
      map.set(tag, list);
    }
  }
  return map;
}

function groupByProject(tasks: TaskDetail[]): Map<string, TaskDetail[]> {
  const map = new Map<string, TaskDetail[]>();
  for (const task of tasks) {
    const key = task.projectName?.trim() || "未分类";
    const list = map.get(key) ?? [];
    list.push(task);
    map.set(key, list);
  }
  return map;
}

/** 全部视图按优先级分组（高/中/低），对齐原版 UI */
const PRIORITY_GROUPS = [
  { priority: 0, label: "高优先级", cls: "high" },
  { priority: 1, label: "中优先级", cls: "mid" },
  { priority: 2, label: "低优先级", cls: "low" },
] as const;
const DELETE_UNDO_SECONDS = 3;

export function TaskPanelView({
  snapshot,
  searchQuery,
  onSearchChange,
  searchResults,
  viewMode,
  onViewModeChange,
  busyId,
  onRefresh,
}: Props) {
  const [newTask, setNewTask] = useState("");
  const [selectedTag, setSelectedTag] = useState<string | null>(null);
  const [selectedProject, setSelectedProject] = useState<string | null>(null);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [undo, setUndo] = useState<{ id: string; title: string } | null>(null);
  const [countdown, setCountdown] = useState(DELETE_UNDO_SECONDS);
  // 正在读秒待删除的任务：先从列表隐藏，读秒结束才真正删除，撤销则恢复且不调后端
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const undoTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const completeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const addInFlightRef = useRef(false);

  function clearTimers() {
    if (undoTimer.current) {
      clearInterval(undoTimer.current);
      undoTimer.current = null;
    }
    if (completeTimer.current) {
      clearTimeout(completeTimer.current);
      completeTimer.current = null;
    }
  }

  // 删除：延迟提交。先隐藏行 + 读秒，到点才 delete_task。
  // 与任务岛一致，撤销期间不触达后端。
  function handleCompleted(task: TaskDetail) {
    clearTimers();
    setPendingDeleteId(task.id);
    setUndo({ id: task.id, title: task.title });
    setCountdown(DELETE_UNDO_SECONDS);
    undoTimer.current = setInterval(() => {
      setCountdown((c) => (c <= 1 ? 0 : c - 1));
    }, 1000);
    completeTimer.current = setTimeout(() => {
      clearTimers();
      void invoke("delete_task", { id: task.id }).then(() => {
        setPendingDeleteId((cur) => (cur === task.id ? null : cur));
        setUndo(null);
        onRefresh();
      });
    }, DELETE_UNDO_SECONDS * 1000);
  }

  function handleUndo() {
    clearTimers();
    setPendingDeleteId(null);
    setUndo(null);
  }

  function toggleExpanded(id: string) {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }

  const isSearching = searchQuery.trim().length > 0;

  const visibleTasks = useMemo(() => {
    const compute = (): TaskDetail[] => {
      if (isSearching && searchResults) {
        return searchResults.incomplete;
      }
      const list = snapshot.incomplete;
      switch (viewMode) {
        case "today":
          return list.filter((t) => t.isInTodayQueue);
        case "suggested":
          return snapshot.suggested;
        case "high":
          return list.filter((t) => t.priority === 0);
        case "upcoming":
          return list
            .filter((t) => t.dueAt)
            .sort((a, b) => (a.dueAt ?? "").localeCompare(b.dueAt ?? ""));
        case "noDate":
          return list.filter((t) => !t.dueAt);
        case "completed":
          return [];
        case "tags":
          if (!selectedTag) return list;
          return list.filter((t) => t.tags.some((tag) => tag === selectedTag));
        case "projects":
          if (!selectedProject) return list;
          return list.filter((t) => (t.projectName?.trim() || "未分类") === selectedProject);
        default:
          return list;
      }
    };
    // 隐藏正在读秒待删除的任务
    return compute().filter((t) => t.id !== pendingDeleteId);
  }, [isSearching, searchResults, snapshot, viewMode, selectedTag, selectedProject, pendingDeleteId]);

  // 分组视图（标签/项目）也需排除待删除任务
  const incompleteVisible = useMemo(
    () => snapshot.incomplete.filter((t) => t.id !== pendingDeleteId),
    [snapshot.incomplete, pendingDeleteId],
  );
  const groupedTagEntries = useMemo(
    () =>
      [...groupByTag(incompleteVisible).entries()].sort(
        ([a, aTasks], [b, bTasks]) =>
          bTasks.length - aTasks.length || a.toLowerCase().localeCompare(b.toLowerCase(), "zh-CN"),
      ),
    [incompleteVisible],
  );

  const showGroupedTags = viewMode === "tags" && selectedTag === null && !isSearching;
  const showGroupedProjects = viewMode === "projects" && selectedProject === null && !isSearching;
  const showPriorityGroups = viewMode === "all" && !isSearching;
  const showFlatList = !showGroupedTags && !showGroupedProjects && !showPriorityGroups;

  async function addTask() {
    if (addInFlightRef.current) return;
    const trimmed = newTask.trim();
    if (!trimmed) return;
    addInFlightRef.current = true;
    try {
      await invoke("quick_add_task", { text: trimmed });
      setNewTask("");
      onRefresh();
    } finally {
      addInFlightRef.current = false;
    }
  }

  return (
    <>
      <FocusCard task={snapshot.focusTask} menuTitle={snapshot.menuBarTitle} onChanged={onRefresh} />

      <form
        className="panel-add-row"
        onSubmit={(e) => {
          e.preventDefault();
          void addTask();
        }}
      >
        <QuickAddInput
          className="panel-add-input"
          placeholder="如 明天 10点 发周报 #工作 !高 /30m（Tab 唤起# ! /）"
          value={newTask}
          onChange={setNewTask}
          knownTags={snapshot.allTagSuggestions}
          knownProjects={snapshot.allProjects}
          onSubmit={() => void addTask()}
        />
        <button type="submit" className="icon-btn panel-add-btn" aria-label="发送">
          <ArrowUp size={16} />
        </button>
      </form>

      <div className="panel-search-row">
        <Search size={14} className="panel-search-icon" />
        <input
          className="panel-search-input"
          placeholder="搜索标题、备注、标签、项目"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
        />
      </div>

      <div className="panel-view-tabs panel-view-tabs-scroll" role="tablist" aria-label="任务视图">
        {PANEL_VIEWS.map((view) => {
          const Icon = view.icon;
          return (
            <button
              key={view.id}
              type="button"
              role="tab"
              aria-selected={viewMode === view.id}
              className={`panel-tab${viewMode === view.id ? " is-active" : ""}`}
              onClick={() => onViewModeChange(view.id)}
            >
              <span className="panel-tab-inner">
                <Icon className="panel-tab-icon" size={14} aria-hidden />
                <span className="panel-tab-label">{view.title}</span>
              </span>
            </button>
          );
        })}
      </div>

      {viewMode === "tags" ? (
        <div className="panel-filter-block">
          <div className="panel-filter-hint">按使用次数显示前 10 个标签</div>
          <TagFilterChips
            tags={snapshot.allTags.filter((t) => t.trim().length > 0)}
            selected={selectedTag}
            onSelect={setSelectedTag}
          />
        </div>
      ) : null}

      {viewMode === "projects" ? (
        <div className="panel-filter-chips">
          <button
            type="button"
            className={`panel-chip-btn${selectedProject === null ? " is-active" : ""}`}
            onClick={() => setSelectedProject(null)}
          >
            全部
          </button>
          {snapshot.allProjects.map((project) => (
            <button
              key={project}
              type="button"
              className={`panel-chip-btn${selectedProject === project ? " is-active" : ""}`}
              onClick={() => setSelectedProject(project)}
            >
              +{project}
            </button>
          ))}
        </div>
      ) : null}

      {showFlatList ? (
        <ul className="panel-task-list">
          {visibleTasks.length === 0 ? (
            <li className="panel-empty">暂无任务</li>
          ) : (
            visibleTasks.map((task) => (
              <TaskRow
                key={task.id}
                task={task}
                busy={busyId === task.id}
                expanded={expandedIds.has(task.id)}
                onToggleExpand={() => toggleExpanded(task.id)}
                onChanged={onRefresh}
                onCompleted={handleCompleted}
              />
            ))
          )}
        </ul>
      ) : null}

      {showPriorityGroups ? (
        <div className="panel-grouped-list">
          {visibleTasks.length === 0 ? (
            <div className="panel-empty">暂无任务</div>
          ) : (
            PRIORITY_GROUPS.map(({ priority, label, cls }) => {
              const tasks = visibleTasks.filter((t) => t.priority === priority);
              if (tasks.length === 0) return null;
              return (
                <section key={priority}>
                  <h4 className="panel-priority-head">
                    <span className={`panel-priority-dot ${cls}`} />
                    {label}
                    <span className="panel-priority-count">{tasks.length}</span>
                  </h4>
                  <ul className="panel-priority-tasks">
                    {tasks.map((task) => (
                      <TaskRow
                        key={task.id}
                        task={task}
                        busy={busyId === task.id}
                        expanded={expandedIds.has(task.id)}
                        onToggleExpand={() => toggleExpanded(task.id)}
                        onChanged={onRefresh}
                        onCompleted={handleCompleted}
                      />
                    ))}
                  </ul>
                </section>
              );
            })
          )}
        </div>
      ) : null}

      {showGroupedTags ? (
        <div className="panel-grouped-list">
          {groupedTagEntries.map(([tag, tasks]) => (
            <section key={tag}>
              <h4>#{tag}</h4>
              <ul className="panel-task-list compact">
                {tasks.map((task) => (
                  <TaskRow
                    key={`${tag}-${task.id}`}
                    task={task}
                    busy={busyId === task.id}
                    expanded={expandedIds.has(task.id)}
                    onToggleExpand={() => toggleExpanded(task.id)}
                    onChanged={onRefresh}
                    onCompleted={handleCompleted}
                  />
                ))}
              </ul>
            </section>
          ))}
        </div>
      ) : null}

      {showGroupedProjects ? (
        <div className="panel-grouped-list">
          {[...groupByProject(incompleteVisible).entries()].map(([project, tasks]) => (
            <section key={project}>
              <h4>+{project}</h4>
              <ul className="panel-task-list compact">
                {tasks.map((task) => (
                  <TaskRow
                    key={`${project}-${task.id}`}
                    task={task}
                    busy={busyId === task.id}
                    expanded={expandedIds.has(task.id)}
                    onToggleExpand={() => toggleExpanded(task.id)}
                    onChanged={onRefresh}
                    onCompleted={handleCompleted}
                  />
                ))}
              </ul>
            </section>
          ))}
        </div>
      ) : null}

      {undo ? (
        <div className="panel-undo-toast">
          <span className="panel-undo-text">待删除：{undo.title}</span>
          <button type="button" className="panel-undo-btn" onClick={() => void handleUndo()}>
            撤销
          </button>
          <span className="panel-undo-count">{countdown}</span>
        </div>
      ) : null}
    </>
  );
}
