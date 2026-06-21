import { invoke } from "@tauri-apps/api/core";
import { Search } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { PanelDateField } from "./PanelDateField";
import { TaskDetail } from "./panelTypes";
import { TaskRow } from "./TaskRow";

/** 无筛选时默认显示的条数 */
const DEFAULT_LIMIT = 10;

/**
 * 历史页：显示已完成/归档任务。
 * 支持与主界面一致的关键词搜索（标题/备注/标签/项目）+ 完成日期范围。
 * 无筛选时默认显示最近 10 条，有筛选时显示全部匹配（按完成时间倒序）。
 */
export function HistoryPanelView() {
  const [query, setQuery] = useState("");
  const [startDate, setStartDate] = useState("");
  const [endDate, setEndDate] = useState("");
  const [activeDateField, setActiveDateField] = useState<"start" | "end" | null>(null);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const toggleExpanded = (id: string) =>
    setExpandedIds((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  const [results, setResults] = useState<TaskDetail[]>([]);

  const load = useCallback(() => {
    void invoke<TaskDetail[]>("query_history", {
      query: query.trim() || null,
      startAt: startDate ? new Date(`${startDate}T00:00:00`).toISOString() : null,
      endAt: endDate ? new Date(`${endDate}T23:59:59`).toISOString() : null,
    })
      .then(setResults)
      .catch(console.error);
  }, [query, startDate, endDate]);

  useEffect(() => {
    const timer = setTimeout(load, 200);
    return () => clearTimeout(timer);
  }, [load]);

  const hasFilter = query.trim().length > 0 || !!startDate || !!endDate;
  const visible = hasFilter ? results : results.slice(0, DEFAULT_LIMIT);

  return (
    <div className="panel-history">
      <div className="panel-search-row">
        <Search size={14} className="panel-search-icon" />
        <input
          className="panel-search-input"
          placeholder="搜索标题、备注、标签"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>

      <div className="panel-history-dates">
        <PanelDateField
          value={startDate}
          includeTime={false}
          open={activeDateField === "start"}
          onOpenChange={(nextOpen) => setActiveDateField(nextOpen ? "start" : null)}
          onChange={(next) => {
            setStartDate(next);
            if (next && endDate && next > endDate) setEndDate(next);
          }}
        />
        <span className="panel-history-dates-sep">至</span>
        <PanelDateField
          value={endDate}
          includeTime={false}
          open={activeDateField === "end"}
          onOpenChange={(nextOpen) => setActiveDateField(nextOpen ? "end" : null)}
          onChange={(next) => {
            setEndDate(next);
            if (next && startDate && next < startDate) setStartDate(next);
          }}
        />
        {hasFilter ? (
          <button
            type="button"
            className="panel-chip-btn"
            onClick={() => {
              setQuery("");
              setStartDate("");
              setEndDate("");
            }}
          >
            清除
          </button>
        ) : null}
      </div>

      <div className="panel-history-hint">
        {hasFilter
          ? `共 ${results.length} 条`
          : results.length > DEFAULT_LIMIT
            ? `默认显示最近 ${DEFAULT_LIMIT} 条 · 用搜索或日期查看更多`
            : `共 ${results.length} 条`}
      </div>

      <ul className="panel-task-list">
        {visible.length === 0 ? (
          <li className="panel-empty">暂无历史记录</li>
        ) : (
          visible.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              busy={false}
              expanded={expandedIds.has(task.id)}
              onToggleExpand={() => toggleExpanded(task.id)}
              onChanged={load}
            />
          ))
        )}
      </ul>
    </div>
  );
}
