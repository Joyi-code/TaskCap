import type { LucideIcon } from "lucide-react";
import {
  Calendar,
  CalendarOff,
  Flag,
  Inbox,
  Sun,
  Tag,
} from "lucide-react";

export type PanelViewId =
  | "all"
  | "today"
  | "suggested"
  | "high"
  | "upcoming"
  | "noDate"
  | "tags"
  | "projects"
  | "completed";

export type PanelViewDef = {
  id: PanelViewId;
  title: string;
  icon: LucideIcon;
};

/** 与 Mac TaskViewMode.systemImage 语义对齐 */
export const PANEL_VIEWS: PanelViewDef[] = [
  { id: "all", title: "全部", icon: Inbox },
  { id: "today", title: "今天", icon: Sun },
  { id: "high", title: "高优", icon: Flag },
  { id: "upcoming", title: "即将", icon: Calendar },
  { id: "tags", title: "标签", icon: Tag },
  { id: "noDate", title: "无日期", icon: CalendarOff },
];
