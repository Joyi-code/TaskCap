import { CalendarDays, ChevronDown, ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";

const WEEKDAYS = ["一", "二", "三", "四", "五", "六", "日"];
const pad = (n: number) => String(n).padStart(2, "0");

// 自定义时间下拉：用纯 div/button 实现，避免原生 <select> 在 WebView2 暗夜模式下
// 弹出列表强制白底（OS 绘制，CSS color-scheme 不可靠）导致文字看不清。
function TimeSelect({
  value,
  count,
  onChange,
}: {
  value: number;
  count: number;
  onChange: (next: number) => void;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    // 打开时将当前选中项滚动到可视区域中部
    const active = listRef.current?.querySelector<HTMLElement>(".is-active");
    active?.scrollIntoView({ block: "center" });
    function onDocPointerDown(event: MouseEvent) {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", onDocPointerDown);
    return () => document.removeEventListener("mousedown", onDocPointerDown);
  }, [open]);

  return (
    <div ref={rootRef} className={`panel-time-select${open ? " is-open" : ""}`}>
      <button
        type="button"
        className="panel-time-select-trigger"
        onClick={(event) => {
          event.stopPropagation();
          setOpen((v) => !v);
        }}
      >
        <span>{pad(value)}</span>
        <ChevronDown size={12} />
      </button>
      {open ? (
        <div ref={listRef} className="panel-time-select-list">
          {Array.from({ length: count }, (_, i) => (
            <button
              type="button"
              key={i}
              className={`panel-time-select-option${i === value ? " is-active" : ""}`}
              onClick={(event) => {
                event.stopPropagation();
                onChange(i);
                setOpen(false);
              }}
            >
              {pad(i)}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function localValueFromDate(d: Date, includeTime: boolean): string {
  const date = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  return includeTime ? `${date}T${pad(d.getHours())}:${pad(d.getMinutes())}` : date;
}

function parseLocalValue(value: string): Date | null {
  if (!value) return null;
  const d = new Date(value.includes("T") ? value : `${value}T00:00:00`);
  return Number.isNaN(d.getTime()) ? null : d;
}

function formatDateButton(value: string, includeTime: boolean): string {
  const d = parseLocalValue(value);
  if (!d) return includeTime ? "选择日期时间" : "选择日期";
  const date = `${d.getMonth() + 1}月${d.getDate()}日`;
  return includeTime ? `${date} ${pad(d.getHours())}:${pad(d.getMinutes())}` : date;
}

function calendarDays(viewMonth: Date): Date[] {
  const first = new Date(viewMonth.getFullYear(), viewMonth.getMonth(), 1);
  const mondayOffset = (first.getDay() + 6) % 7;
  const start = new Date(first);
  start.setDate(first.getDate() - mondayOffset);
  return Array.from({ length: 42 }, (_, i) => {
    const d = new Date(start);
    d.setDate(start.getDate() + i);
    return d;
  });
}

type Props = {
  value: string;
  onChange: (value: string) => void;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  includeTime?: boolean;
  clearLabel?: string;
};

export function PanelDateField({
  value,
  onChange,
  open,
  onOpenChange,
  includeTime = true,
  clearLabel = "清除",
}: Props) {
  const selected = parseLocalValue(value);
  const [viewMonth, setViewMonth] = useState(() => selected ?? new Date());
  // 未设置时间时，时间面板默认显示当前系统时间（而非固定 9:00）
  const now = new Date();
  const hour = selected?.getHours() ?? now.getHours();
  const minute = selected?.getMinutes() ?? now.getMinutes();

  useEffect(() => {
    if (selected) setViewMonth(selected);
  }, [value]);

  function commit(next: Date) {
    onChange(localValueFromDate(next, includeTime));
  }

  function commitDate(day: Date) {
    const next = new Date(day);
    next.setHours(hour, minute, 0, 0);
    commit(next);
    if (!includeTime) onOpenChange(false);
  }

  function commitTime(nextHour: number, nextMinute: number) {
    const next = selected ?? new Date();
    next.setHours(nextHour, nextMinute, 0, 0);
    commit(next);
  }

  const days = calendarDays(viewMonth);
  const today = new Date();

  return (
    <div className="panel-date-control" onClick={(event) => event.stopPropagation()}>
      <button
        type="button"
        className={`panel-date-shell${open ? " is-open" : ""}`}
        onClick={() => onOpenChange(!open)}
      >
        <CalendarDays size={13} />
        <span>{formatDateButton(value, includeTime)}</span>
        <ChevronDown size={13} />
      </button>

      {open ? (
        <div className="panel-date-popover">
          <div className="panel-date-popover-head">
            <button
              type="button"
              className="panel-date-nav"
              onClick={() => setViewMonth(new Date(viewMonth.getFullYear(), viewMonth.getMonth() - 1, 1))}
            >
              <ChevronLeft size={13} />
            </button>
            <strong>{viewMonth.getFullYear()}年{viewMonth.getMonth() + 1}月</strong>
            <button
              type="button"
              className="panel-date-nav"
              onClick={() => setViewMonth(new Date(viewMonth.getFullYear(), viewMonth.getMonth() + 1, 1))}
            >
              <ChevronRight size={13} />
            </button>
          </div>

          <div className="panel-date-weekdays">
            {WEEKDAYS.map((day) => <span key={day}>{day}</span>)}
          </div>
          <div className="panel-date-grid">
            {days.map((day) => {
              const isMuted = day.getMonth() !== viewMonth.getMonth();
              const isSelected =
                !!selected &&
                day.getFullYear() === selected.getFullYear() &&
                day.getMonth() === selected.getMonth() &&
                day.getDate() === selected.getDate();
              const isToday =
                day.getFullYear() === today.getFullYear() &&
                day.getMonth() === today.getMonth() &&
                day.getDate() === today.getDate();
              return (
                <button
                  key={day.toISOString()}
                  type="button"
                  className={`panel-date-day${isMuted ? " is-muted" : ""}${isSelected ? " is-selected" : ""}${isToday ? " is-today" : ""}`}
                  onClick={() => commitDate(day)}
                >
                  {day.getDate()}
                </button>
              );
            })}
          </div>

          {includeTime ? (
            <div className="panel-date-time-row">
              <TimeSelect
                value={hour}
                count={24}
                onChange={(next) => commitTime(next, minute)}
              />
              <span>:</span>
              <TimeSelect
                value={minute}
                count={60}
                onChange={(next) => commitTime(hour, next)}
              />
            </div>
          ) : null}

          <div className="panel-date-footer">
            <button type="button" onClick={() => { onChange(""); onOpenChange(false); }}>{clearLabel}</button>
            <button type="button" onClick={() => commit(new Date())}>今天</button>
            <button type="button" className="primary" onClick={() => {
              if (!value) {
                const next = new Date();
                next.setHours(hour, minute, 0, 0);
                commit(next);
              }
              onOpenChange(false);
            }}>完成</button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
