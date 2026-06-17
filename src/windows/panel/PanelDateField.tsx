import { CalendarDays, ChevronDown, ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useState } from "react";

const WEEKDAYS = ["一", "二", "三", "四", "五", "六", "日"];
const pad = (n: number) => String(n).padStart(2, "0");

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
              <select
                value={hour}
                onChange={(e) => commitTime(Number(e.target.value), minute)}
              >
                {Array.from({ length: 24 }, (_, i) => (
                  <option key={i} value={i}>{pad(i)}</option>
                ))}
              </select>
              <span>:</span>
              <select
                value={minute}
                onChange={(e) => commitTime(hour, Number(e.target.value))}
              >
                {Array.from({ length: 60 }, (_, i) => (
                  <option key={i} value={i}>{pad(i)}</option>
                ))}
              </select>
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
