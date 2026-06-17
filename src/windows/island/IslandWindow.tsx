import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Bell, Check, Pause, Pin, Play, Plus, Square, Timer, Trash2, Undo2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import "../../styles/glass.css";
import "../../styles/island.css";
import { createPointerDragTracker } from "../../lib/windowDrag";
import { ensureIslandAlwaysOnTop } from "../../lib/windowZOrder";
import {
  animateIslandWindowSize,
  applyIslandWindowSize,
  cancelIslandResizeAnimation,
  IslandShellMode,
  resolveIslandSize,
  windowMatchesTarget,
} from "./islandLayout";
import { formatIslandTaskMeta, type IslandTaskSummary } from "../../lib/islandTaskMeta";
import { useUiTransparency } from "../../lib/useUiTransparency";
import { readPanelSettings, type PanelSettings } from "../panel/usePanelSettings";
import { useAppConfig } from "../panel/useAppConfig";

const PANEL_SETTINGS_KEY = "taskcap.panel.settings";

type TaskCounts = {
  high: number;
  medium: number;
  low: number;
  total: number;
};

type TaskSummary = IslandTaskSummary;

type IslandSnapshot = {
  focusCounts: TaskCounts;
  menuBarTitle: string;
  attentionTask: TaskSummary | null;
  previewTasks: TaskSummary[];
  expandedHeight: number;
  hasActiveFocus: boolean;
  incompleteCount: number;
};

const PRIORITY_DOT: Record<number, string> = {
  0: "priority-high",
  1: "priority-medium",
  2: "priority-low",
};

const PRIORITY_SHORT_TITLE: Record<number, string> = {
  0: "高",
  1: "中",
  2: "低",
};

const EMPTY_SNAPSHOT: IslandSnapshot = {
  focusCounts: { high: 0, medium: 0, low: 0, total: 0 },
  menuBarTitle: "完成",
  attentionTask: null,
  previewTasks: [],
  expandedHeight: 92,
  hasActiveFocus: false,
  incompleteCount: 0,
};

// 任务提醒（截止/提醒到点）展示时长：到点后展示 60 秒自动让位，喝水/久坐则不自动消失
const REMINDER_ATTENTION_MS = 60_000;
const DELETE_UNDO_SECONDS = 3;

/** 统一提醒项：喝水 / 久坐 / 任务（截止或提醒到点），进同一条队列依次展示 */
type ReminderItem =
  | { kind: "water" }
  | { kind: "sitting" }
  | { kind: "task"; task: TaskSummary };

/** 两个提醒是否视为同一项（用于去重）：同类即同项，任务再比对 id */
function sameReminder(a: ReminderItem, b: ReminderItem): boolean {
  if (a.kind !== b.kind) return false;
  if (a.kind === "task" && b.kind === "task") return a.task.id === b.task.id;
  return true;
}

function isInteractiveTarget(target: EventTarget | null): boolean {
  return target instanceof Element && !!target.closest(
    ".island-preview-action, .island-action-btn, .island-attention-action-btn, input, textarea, select",
  );
}

function formatFocusCountdown(seconds: number | null | undefined): string {
  const total = Math.max(Math.round(seconds ?? 0), 0);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const remainingSeconds = total % 60;
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(remainingSeconds).padStart(2, "0")}`;
  }
  return `${minutes}:${String(remainingSeconds).padStart(2, "0")}`;
}

function resolveAttentionTask(
  snapshot: IslandSnapshot,
  reminderTask: TaskSummary | null,
): TaskSummary | null {
  return reminderTask ?? snapshot.attentionTask;
}

function shellMode(
  isExpanded: boolean,
  attentionTask: TaskSummary | null,
): IslandShellMode {
  if (isExpanded) return "expanded";
  if (attentionTask) return "attention";
  return "collapsed";
}

/** 岛窗口只有一个实例，用 module 级守卫确保「定位+显示」只触发一次 */
let islandReadyTriggered = false;

/** 等字体与两帧绘制完成后再 show，减轻打包版 WebView2 透明边框闪缩 */
async function revealIslandWhenPaintReady(width: number, height: number) {
  try {
    await document.fonts.ready;
  } catch {
    /* 字体 API 不可用时跳过 */
  }
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
  await invoke("island_ready", { width, height });
}

/** 三态悬浮岛 — collapsed 172×30 / attention 340×52 / expanded 440×92（单行轮播） */
export function IslandWindow() {
  const [snapshot, setSnapshot] = useState<IslandSnapshot>(EMPTY_SNAPSHOT);
  const [isExpanded, setIsExpanded] = useState(false);
  const [showsExpandedContent, setShowsExpandedContent] = useState(false);
  const [isPinned, setIsPinned] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [reminderTask, setReminderTask] = useState<TaskSummary | null>(null);
  const [panelSettings, setPanelSettings] = useState<PanelSettings>(readPanelSettings);
  const [clockTick, setClockTick] = useState(0);
  const { glassStyle: islandGlassStyle } = useUiTransparency(panelSettings.darkGlassMode);
  const { config: appConfig } = useAppConfig();
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [pendingDeleteSecsLeft, setPendingDeleteSecsLeft] = useState(DELETE_UNDO_SECONDS);
  const [customReminder, setCustomReminder] = useState<{ type: "water" | "sitting" } | null>(null);
  const customReminderRef = useRef<{ type: "water" | "sitting" } | null>(null);
  // 统一提醒队列：pending 排队项 + 当前展示项 + 两个稳定回调引用（供 interval/事件监听内调用，避免重建定时器）
  const reminderQueueRef = useRef<ReminderItem[]>([]);
  const activeReminderRef = useRef<ReminderItem | null>(null);
  const dismissActiveReminderRef = useRef<() => void>(() => {});
  const enqueueReminderRef = useRef<(item: ReminderItem) => void>(() => {});
  const collapseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reminderTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingDeleteTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingDeleteCountdown = useRef<ReturnType<typeof setInterval> | null>(null);
  const islandDrag = useRef(createPointerDragTracker());
  const layoutModeRef = useRef<IslandShellMode>("collapsed");
  const lastSizeRef = useRef<{ width: number; height: number } | null>(null);
  const collapseGeneration = useRef(0);
  const layoutBusy = useRef(false);
  const isExpandedRef = useRef(false);
  const showsExpandedRef = useRef(false);
  const snapshotRef = useRef(snapshot);
  const dataReadyRef = useRef(false);
  const expandGeneration = useRef(0);

  const refresh = useCallback(async () => {
    const data = await invoke<IslandSnapshot>("get_island_snapshot", { defaultFocusMinutes: readPanelSettings().defaultFocusMinutes });
    setSnapshot(data);
    return data;
  }, []);

  const syncWindowSize = useCallback(
    async (mode: IslandShellMode, expandedHeight: number, animated: boolean) => {
      const { width, height } = resolveIslandSize(mode, expandedHeight);
      if (await windowMatchesTarget(width, height)) {
        layoutModeRef.current = mode;
        lastSizeRef.current = { width, height };
        return;
      }

      layoutBusy.current = true;
      try {
        if (animated) {
          await animateIslandWindowSize(width, height);
        } else {
          await applyIslandWindowSize(width, height);
        }
        layoutModeRef.current = mode;
        lastSizeRef.current = { width, height };
      } finally {
        layoutBusy.current = false;
      }
    },
    [],
  );

  const applyLayout = useCallback(
    async (
      expanded: boolean,
      data: IslandSnapshot,
      animated: boolean,
      reminder: TaskSummary | null = reminderTask,
    ) => {
      const mode = shellMode(expanded, resolveAttentionTask(data, reminder));
      await syncWindowSize(mode, data.expandedHeight, animated);
    },
    [reminderTask, syncWindowSize],
  );

  const postponePanelPreload = useCallback((millis = 30_000) => {
    void invoke("postpone_panel_preload", { millis }).catch(() => undefined);
  }, []);

  /** 岛展开动画/预加载期间急点「+」：先停动画、落稳尺寸，再同步打开快速新增 */
  const openQuickAdd = useCallback(async () => {
    postponePanelPreload();
    cancelIslandResizeAnimation();
    const data = snapshotRef.current;
    if (isExpandedRef.current) {
      try {
        await applyLayout(true, data, false);
      } catch (error) {
        console.error(error);
      }
    }
    try {
      await invoke("show_quickadd");
    } catch (error) {
      console.error(error);
    }
  }, [applyLayout, postponePanelPreload]);

  const expandIsland = useCallback(async () => {
    if (isExpanded && showsExpandedContent) return;

    postponePanelPreload();
    collapseTimer.current && clearTimeout(collapseTimer.current);
    const generation = ++expandGeneration.current;

    let data = snapshotRef.current;
    if (!dataReadyRef.current) {
      try {
        data = await refresh();
        dataReadyRef.current = true;
      } catch {
        return;
      }
    } else {
      void refresh();
    }
    if (generation !== expandGeneration.current) return;
    if (data.incompleteCount === 0) return;
    if (data.attentionTask) return;

    setIsExpanded(true);
    await applyLayout(true, data, true);
    if (generation !== expandGeneration.current) return;
    setShowsExpandedContent(true);
  }, [applyLayout, isExpanded, postponePanelPreload, refresh, showsExpandedContent]);

  const collapseIsland = useCallback(async () => {
    postponePanelPreload();
    cancelIslandResizeAnimation();
    expandGeneration.current += 1;
    collapseTimer.current && clearTimeout(collapseTimer.current);
    const generation = ++collapseGeneration.current;
    const data = snapshotRef.current;

    setShowsExpandedContent(false);
    setIsExpanded(false);
    // 急点关闭：跳过动画，避免与进行中的 expand resize 争主线程
    await applyLayout(false, data, false);
    void refresh().then((fresh) => {
      if (generation !== collapseGeneration.current) return;
      setSnapshot(fresh);
    });
  }, [applyLayout, postponePanelPreload, refresh]);

  const scheduleCollapse = useCallback(() => {
    if (isPinned || isDragging || customReminderRef.current !== null) return;
    collapseTimer.current && clearTimeout(collapseTimer.current);
    collapseTimer.current = setTimeout(() => {
      void collapseIsland();
    }, 3000);
  }, [collapseIsland, isDragging, isPinned]);

  const requestExpandFromPointer = useCallback(() => {
    collapseTimer.current && clearTimeout(collapseTimer.current);
  }, []);

  function startPendingDelete(id: string) {
    if (pendingDeleteTimer.current) clearTimeout(pendingDeleteTimer.current);
    if (pendingDeleteCountdown.current) clearInterval(pendingDeleteCountdown.current);
    setPendingDeleteId(id);
    setPendingDeleteSecsLeft(DELETE_UNDO_SECONDS);
    pendingDeleteCountdown.current = setInterval(() => {
      setPendingDeleteSecsLeft((s) => s - 1);
    }, 1000);
    pendingDeleteTimer.current = setTimeout(() => {
      if (pendingDeleteCountdown.current) clearInterval(pendingDeleteCountdown.current);
      void invoke("delete_task", { id }).then(() =>
        refresh().then((data) => applyLayout(isExpandedRef.current, data, false)),
      ).finally(() => {
        setPendingDeleteId((cur) => (cur === id ? null : cur));
        setPendingDeleteSecsLeft(DELETE_UNDO_SECONDS);
      });
    }, DELETE_UNDO_SECONDS * 1000);
  }

  function cancelPendingDelete() {
    if (pendingDeleteTimer.current) clearTimeout(pendingDeleteTimer.current);
    if (pendingDeleteCountdown.current) clearInterval(pendingDeleteCountdown.current);
    setPendingDeleteId(null);
    setPendingDeleteSecsLeft(DELETE_UNDO_SECONDS);
  }

  function dismissCustomReminder() {
    void dismissActiveReminderRef.current();
  }

  const persistIslandPosition = useCallback(async () => {
    const win = getCurrentWindow();
    const pos = await win.outerPosition();
    const size = await win.outerSize();
    await invoke("save_island_position", {
      x: pos.x,
      y: pos.y,
      width: size.width,
    });
  }, []);

  const handleIslandPointerDown = (event: React.PointerEvent) => {
    postponePanelPreload();
    if (isInteractiveTarget(event.target)) return;
    if (!islandDrag.current.onPointerDown(event)) return;
    setIsDragging(true);
  };

  const handleIslandPointerUp = () => {
    const dragged = islandDrag.current.onPointerUp();
    setIsDragging(false);
    if (dragged) {
      void persistIslandPosition();
    }
    scheduleCollapse();
  };

  const handleIslandClick = () => {
    if (!islandDrag.current.wasClick()) {
      islandDrag.current.resetClickGuard();
      return;
    }
    void invoke("toggle_panel");
  };

  const handleCollapsedClick = () => {
    if (!islandDrag.current.wasClick()) {
      islandDrag.current.resetClickGuard();
      return;
    }
    if (snapshot.incompleteCount === 0) return;
    void expandIsland();
  };

  // === 统一提醒队列控制器 ===
  // 喝水、久坐、任务截止/提醒到点都进同一条队列，同一时刻只展示一个；
  // 关闭当前提醒后立即弹出下一个，杜绝「同时到点只剩一个、其余被丢弃」。
  const activateReminder = useCallback(
    async (item: ReminderItem) => {
      // 同步占位，保证并发入队能立即看到「已有展示项」而排队
      activeReminderRef.current = item;
      reminderTimer.current && clearTimeout(reminderTimer.current);
      collapseTimer.current && clearTimeout(collapseTimer.current);

      if (item.kind === "task") {
        setCustomReminder(null);
        setReminderTask(item.task);
        setIsExpanded(false);
        setShowsExpandedContent(false);
        const data = await refresh();
        await applyLayout(false, data, true, item.task);
        // 任务提醒展示固定时长后自动让位给队列里的下一个
        reminderTimer.current = setTimeout(() => {
          dismissActiveReminderRef.current();
        }, REMINDER_ATTENTION_MS);
      } else {
        // 喝水 / 久坐：窗口尺寸由 customReminder 专属 effect 锁定，保持到用户点击关闭
        setReminderTask(null);
        setCustomReminder({ type: item.kind });
      }
    },
    [applyLayout, refresh],
  );

  const advanceReminderQueue = useCallback(async () => {
    const queue = reminderQueueRef.current;
    // 任务提醒优先级高于喝水/久坐：先取最早入队的任务提醒，没有任务提醒再取软提醒
    let index = queue.findIndex((item) => item.kind === "task");
    if (index === -1 && queue.length > 0) index = 0;
    if (index === -1) {
      activeReminderRef.current = null;
      return;
    }
    const [next] = queue.splice(index, 1);
    await activateReminder(next);
  }, [activateReminder]);

  const dismissActiveReminder = useCallback(async () => {
    reminderTimer.current && clearTimeout(reminderTimer.current);
    // 队列还有待展示项：直接切到下一个，无需先收起
    if (reminderQueueRef.current.length > 0) {
      await advanceReminderQueue();
      return;
    }
    // 队列已空：清空提醒态并恢复收起
    activeReminderRef.current = null;
    setReminderTask(null);
    setCustomReminder(null);
    setIsExpanded(false);
    setShowsExpandedContent(false);
    const data = await refresh();
    await applyLayout(false, data, true, null);
  }, [advanceReminderQueue, applyLayout, refresh]);

  const enqueueReminder = useCallback(
    (item: ReminderItem) => {
      // 去重：已在展示或已在队列中的同类提醒不重复入队
      const active = activeReminderRef.current;
      if (active && sameReminder(active, item)) return;
      if (reminderQueueRef.current.some((queued) => sameReminder(queued, item))) return;

      if (!active) {
        void activateReminder(item);
        return;
      }

      // 任务提醒优先级高于喝水/久坐：到点时打断正在显示的软提醒并立即插队展示；
      // 被打断的软提醒放回队列，任务走完后恢复显示，直到人为点掉
      if (item.kind === "task" && active.kind !== "task") {
        reminderQueueRef.current.push(active);
        void activateReminder(item);
        return;
      }

      reminderQueueRef.current.push(item);
    },
    [activateReminder],
  );

  // 把最新回调写入 ref，供 interval / 事件监听内稳定调用，避免改写依赖导致定时器/监听重建
  useEffect(() => {
    dismissActiveReminderRef.current = () => {
      void dismissActiveReminder();
    };
    enqueueReminderRef.current = enqueueReminder;
  }, [dismissActiveReminder, enqueueReminder]);

  useEffect(() => {
    isExpandedRef.current = isExpanded;
    showsExpandedRef.current = showsExpandedContent;
  }, [isExpanded, showsExpandedContent]);

  useEffect(() => {
    snapshotRef.current = snapshot;
  }, [snapshot]);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key === PANEL_SETTINGS_KEY) {
        setPanelSettings(readPanelSettings());
      }
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  useEffect(() => {
    customReminderRef.current = customReminder;
  }, [customReminder]);

  useEffect(() => {
    if (!panelSettings.waterReminderEnabled) return;
    const ms = panelSettings.waterReminderMinutes * 60_000;
    const timer = setInterval(() => {
      enqueueReminderRef.current({ kind: "water" });
    }, ms);
    return () => clearInterval(timer);
  }, [panelSettings.waterReminderEnabled, panelSettings.waterReminderMinutes]);

  useEffect(() => {
    if (!panelSettings.sittingReminderEnabled) return;
    const ms = panelSettings.sittingReminderMinutes * 60_000;
    const timer = setInterval(() => {
      enqueueReminderRef.current({ kind: "sitting" });
    }, ms);
    return () => clearInterval(timer);
  }, [panelSettings.sittingReminderEnabled, panelSettings.sittingReminderMinutes]);

  useEffect(() => {
    if (customReminder !== null) {
      collapseTimer.current && clearTimeout(collapseTimer.current);
      if (appConfig.displayMode === "wide") {
        void syncWindowSize("attention", 0, true);
      } else {
        void syncWindowSize("collapsed", 0, true);
      }
    }
  }, [customReminder, appConfig.displayMode, syncWindowSize]);

  useEffect(() => {
    const onPointerProbe = (event: PointerEvent | MouseEvent) => {
      if ((event.target as Element | null)?.closest?.(".island-shell")) {
        requestExpandFromPointer();
      }
    };
    document.addEventListener("pointermove", onPointerProbe);
    document.addEventListener("mousemove", onPointerProbe);
    return () => {
      document.removeEventListener("pointermove", onPointerProbe);
      document.removeEventListener("mousemove", onPointerProbe);
    };
  }, [requestExpandFromPointer]);

  useEffect(() => {
    void ensureIslandAlwaysOnTop();
    // 启动即以收起态尺寸定位并显示岛，不等数据库查询返回。
    // 首次启动 DB 初始化可能耗时数秒，若等 refresh 才 show，岛会迟迟不出现
    // （表现为启动「一闪一闪 / 黑屏后才弹出」）。先用已知的 collapsed 尺寸立刻显示。
    if (!islandReadyTriggered) {
      islandReadyTriggered = true;
      const { width, height } = resolveIslandSize("collapsed");
      layoutModeRef.current = "collapsed";
      lastSizeRef.current = { width, height };
      // 等浏览器完成首帧绘制后再通知后端定位+显示，避免打包版透明边框闪缩
      void revealIslandWhenPaintReady(width, height).catch(console.error);
    }
    // 数据异步加载，返回后再校正内容与尺寸（如启动即有提醒/专注任务时切到 attention 态）
    refresh()
      .then((data) => {
        dataReadyRef.current = true;
        const mode = shellMode(false, resolveAttentionTask(data, null));
        const { width, height } = resolveIslandSize(mode, data.expandedHeight);
        layoutModeRef.current = mode;
        lastSizeRef.current = { width, height };
        if (mode !== "collapsed") {
          void applyLayout(false, data, false);
        }
      })
      .catch(console.error);
    const unlistenTasks = listen("tasks-changed", () => {
      refresh()
        .then(async (data) => {
          if (layoutBusy.current) return;
          // 提醒态（久坐/喝水）窗口尺寸由专属 effect 锁定，任务刷新不得改尺寸，
          // 否则会出现尺寸与圆角错配、四周露出矩形背景
          if (customReminderRef.current !== null) return;
          const expanded = isExpandedRef.current && showsExpandedRef.current;
          if (expanded) {
            const { width, height } = resolveIslandSize("expanded", data.expandedHeight);
            if (!(await windowMatchesTarget(width, height))) {
              await applyLayout(true, data, false);
            }
            return;
          }
          await applyLayout(isExpandedRef.current, data, false);
        })
        .catch(console.error);
    });
    const unlistenReminder = listen<string>("reminder-due", async (event) => {
      const taskId = event.payload;
      try {
        const tasks = await invoke<TaskSummary[]>("list_incomplete_tasks", { defaultFocusMinutes: readPanelSettings().defaultFocusMinutes });
        const task = tasks.find((item) => item.id === taskId) ?? null;
        if (!task) return;
        // 入统一队列：多任务同一时刻到点不再互相覆盖，依次展示
        enqueueReminderRef.current({ kind: "task", task });
      } catch (error) {
        console.error(error);
      }
    });
    return () => {
      collapseTimer.current && clearTimeout(collapseTimer.current);
      reminderTimer.current && clearTimeout(reminderTimer.current);
      pendingDeleteTimer.current && clearTimeout(pendingDeleteTimer.current);
      pendingDeleteCountdown.current && clearInterval(pendingDeleteCountdown.current);
      void unlistenTasks.then((off) => off());
      void unlistenReminder.then((off) => off());
    };
  }, [applyLayout, refresh]);

  async function runPreviewAction(id: string, action: "complete" | "archive") {
    collapseTimer.current && clearTimeout(collapseTimer.current);
    if (action === "archive") {
      startPendingDelete(id);
      return;
    }
    setBusyId(id);
    try {
      await invoke("complete_task", { id });
      const data = await refresh();
      await applyLayout(isExpandedRef.current, data, false);
    } finally {
      setBusyId(null);
    }
  }

  const attentionTask = resolveAttentionTask(snapshot, reminderTask);
  const isReminderAttention = !snapshot.attentionTask && reminderTask !== null;
  const mode = shellMode(isExpanded, attentionTask);
  const isFocusTimeUp = !!(
    snapshot.attentionTask?.isFocusRunning &&
    snapshot.attentionTask.focusRemainingSeconds != null &&
    snapshot.attentionTask.focusRemainingSeconds - clockTick <= 0
  );

  const showsCustomReminder = customReminder !== null;
  const showsExpanded = !showsCustomReminder && isExpanded && showsExpandedContent;
  const showsAttention = !showsCustomReminder && !showsExpanded && attentionTask !== null;
  const showsCollapsed = !showsCustomReminder && !showsExpanded && !showsAttention;

  const expandedLayerRef = useRef<HTMLDivElement>(null);
  const previewTasks = snapshot.previewTasks.slice(0, 3);

  // 提醒态的窗口尺寸由 displayMode 决定（标准=收起 172×30，宽大=专注 340×52），
  // 圆角必须与该尺寸匹配，否则高框配小圆角会露出矩形背景
  const reminderMode: IslandShellMode | null = showsCustomReminder
    ? appConfig.displayMode === "wide"
      ? "attention"
      : "collapsed"
    : null;
  const effectiveMode = reminderMode ?? mode;
  const cornerClass =
    effectiveMode === "collapsed" ? "island-radius-collapsed" : effectiveMode === "attention" ? "island-radius-attention" : "island-radius-expanded";

  useEffect(() => {
    document.documentElement.classList.add("island-root");
    return () => document.documentElement.classList.remove("island-root");
  }, []);

  const renderCollapsedBody = () => (
    <span className="island-priority-row">
      {(
        [
          [0, snapshot.focusCounts.high],
          [1, snapshot.focusCounts.medium],
          [2, snapshot.focusCounts.low],
        ] as const
      ).map(([priority, count]) => (
        <span key={priority} className="island-priority-chip">
          <span className={`island-dot ${PRIORITY_DOT[priority]}`} />
          <span className="island-priority-num">{count}</span>
        </span>
      ))}
    </span>
  );

  async function runAttentionAction(task: TaskSummary, action: "start" | "pause" | "stop") {
    setBusyId(task.id);
    try {
      if (action === "start") {
        await invoke("start_focus", { id: task.id });
      } else if (action === "pause") {
        await invoke("pause_focus", { id: task.id });
      } else {
        await invoke("stop_focus", { id: task.id });
      }
      const data = await refresh();
      await applyLayout(false, data, false);
    } finally {
      setBusyId(null);
    }
  }

  function attentionSubtitle(task: TaskSummary): string {
    if (snapshot.attentionTask) {
      const state = task.isFocusRunning ? "专注中" : "已暂停";
      const base = task.focusRemainingSeconds;
      if (base != null) {
        const remaining = Math.max(task.isFocusRunning ? base - clockTick : base, 0);
        return `${state}，剩余 ${formatFocusCountdown(remaining)}`;
      }
      return `${state} · ${PRIORITY_SHORT_TITLE[task.priority] ?? "中"}优先级`;
    }
    return formatIslandTaskMeta(task) ?? `提醒到了 · ${PRIORITY_SHORT_TITLE[task.priority] ?? "中"}优先级`;
  }

  function attentionTrailingText(task: TaskSummary): string {
    if (snapshot.attentionTask) return "";
    return "现在";
  }

  useEffect(() => {
    if (!attentionTask?.isFocusRunning) {
      setClockTick(0);
      return;
    }
    setClockTick(0);
    const timer = setInterval(() => setClockTick((value) => value + 1), 1000);
    return () => clearInterval(timer);
  }, [attentionTask?.id, attentionTask?.isFocusRunning, attentionTask?.focusRemainingSeconds]);

  // 每分钟 refresh 一次，让超时状态实时更新
  useEffect(() => {
    const timer = setInterval(() => {
      void refresh();
    }, 60_000);
    return () => clearInterval(timer);
  }, [refresh]);

  useEffect(() => {
    if (!snapshot.attentionTask?.isFocusRunning) return;
    const timer = setInterval(() => {
      // 提醒态期间不刷新布局，避免改动窗口尺寸破坏提醒岛台圆角
      if (customReminderRef.current !== null) return;
      refresh()
        .then((data) => applyLayout(false, data, false))
        .catch(console.error);
    }, 15_000);
    return () => clearInterval(timer);
  }, [applyLayout, refresh, snapshot.attentionTask?.id, snapshot.attentionTask?.isFocusRunning]);

  return (
    <div
      className={`island-shell glass-surface ${cornerClass}${panelSettings.darkGlassMode ? " island-dark" : ""}${isFocusTimeUp ? " island-focus-ended" : ""}`}
      style={islandGlassStyle}
      onMouseEnter={requestExpandFromPointer}
      onPointerEnter={requestExpandFromPointer}
      onPointerMove={requestExpandFromPointer}
      onMouseLeave={() => { if (!busyId && !pendingDeleteId) scheduleCollapse(); }}
    >
      <div className={`island-layer${showsCollapsed ? " is-visible" : ""}`}>
        <button
          type="button"
          className="island-collapsed-btn"
          onPointerEnter={requestExpandFromPointer}
          onPointerDown={handleIslandPointerDown}
          onPointerMove={(e) => {
            islandDrag.current.onPointerMove(e);
            requestExpandFromPointer();
          }}
          onPointerUp={handleIslandPointerUp}
          onPointerCancel={handleIslandPointerUp}
          onClick={handleCollapsedClick}
          title={snapshot.incompleteCount === 0 ? "暂无任务" : "单击展开 TaskCap；拖动移动"}
        >
          {renderCollapsedBody()}
        </button>
      </div>

      <div className={`island-layer${showsAttention ? " is-visible" : ""}`}>
        {attentionTask ? (
          <div
            className={`island-attention-btn${isReminderAttention ? " is-reminder" : ""}`}
            onPointerDown={handleIslandPointerDown}
            onPointerMove={(e) => islandDrag.current.onPointerMove(e)}
            onPointerUp={handleIslandPointerUp}
            onPointerCancel={handleIslandPointerUp}
            onClick={handleIslandClick}
            title="拖动移动；单击打开任务面板"
            role="button"
            tabIndex={0}
          >
            <span className={`island-attention-icon ${PRIORITY_DOT[attentionTask.priority]}`}>
              {snapshot.hasActiveFocus ? <Timer size={14} /> : <Bell size={14} />}
            </span>
            <span className="island-attention-copy">
              <span className="island-attention-title">{attentionTask.title}</span>
              <span className="island-attention-sub">
                {attentionSubtitle(attentionTask)}
              </span>
            </span>
            {isReminderAttention ? (
              <span className="island-attention-actions" aria-label="提醒操作">
                <span className="island-attention-badge">
                  {attentionTrailingText(attentionTask)}
                </span>
                <button
                  type="button"
                  className="island-attention-action-btn"
                  aria-label="关闭提醒"
                  title="关闭提醒"
                  onClick={(event) => {
                    event.stopPropagation();
                    dismissActiveReminderRef.current();
                  }}
                >
                  <X size={11} />
                </button>
              </span>
            ) : null}
            {snapshot.attentionTask ? (
              <span className="island-attention-actions" aria-label="专注控制">
                <button
                  type="button"
                  className="island-attention-action-btn"
                  aria-label={attentionTask.isFocusRunning ? "暂停专注" : "继续专注"}
                  disabled={busyId === attentionTask.id}
                  onClick={(event) => {
                    event.stopPropagation();
                    void runAttentionAction(attentionTask, attentionTask.isFocusRunning ? "pause" : "start");
                  }}
                >
                  {attentionTask.isFocusRunning ? <Pause size={11} /> : <Play size={11} />}
                </button>
                <button
                  type="button"
                  className="island-attention-action-btn"
                  aria-label="停止专注"
                  disabled={busyId === attentionTask.id}
                  onClick={(event) => {
                    event.stopPropagation();
                    void runAttentionAction(attentionTask, "stop");
                  }}
                >
                  <Square size={10} fill="currentColor" />
                </button>
              </span>
            ) : null}
          </div>
        ) : null}
      </div>

      <div className={`island-layer${showsExpanded ? " is-visible" : ""}`}>
        <div
          ref={expandedLayerRef}
          className="island-expanded-layout"
          title={previewTasks.length > 1 ? "最多显示 3 条重点任务" : undefined}
          onPointerDown={handleIslandPointerDown}
          onPointerMove={(e) => islandDrag.current.onPointerMove(e)}
          onPointerUp={handleIslandPointerUp}
          onPointerCancel={handleIslandPointerUp}
          onClick={(event) => {
            if (isInteractiveTarget(event.target)) return;
            handleIslandClick();
          }}
        >
          <div className="island-preview-list">
            {snapshot.incompleteCount === 0 ? (
              <div className="island-preview-empty">全部完成</div>
            ) : (
              previewTasks.map((task) => {
                if (task.id === pendingDeleteId) {
                  return (
                    <div key={task.id} className="island-preview-row island-preview-row-deleting">
                      <span className="island-preview-title island-preview-title-deleting">{task.title}</span>
                      <span className="island-delete-countdown">{pendingDeleteSecsLeft}s</span>
                      <button
                        type="button"
                        className="island-preview-action"
                        aria-label="撤销删除"
                        onClick={(event) => { event.stopPropagation(); cancelPendingDelete(); }}
                      >
                        <Undo2 size={12} />
                      </button>
                    </div>
                  );
                }
                const meta = formatIslandTaskMeta(task);
                return (
                  <div
                    key={task.id}
                    className={`island-preview-row${task.isCurrent ? " is-current" : ""}`}
                  >
                    <span className={`island-dot ${PRIORITY_DOT[task.priority]}`} />
                    <span className="island-preview-title">{task.title}</span>
                    {task.isCurrent ? <span className="island-preview-current">当前</span> : null}
                    {meta ? (
                      <span className="island-preview-meta">{meta}</span>
                    ) : null}
                    <button
                      type="button"
                      className="island-preview-action"
                      disabled={busyId === task.id}
                      aria-label="归档"
                      title="归档"
                      onClick={(event) => {
                        event.stopPropagation();
                        void runPreviewAction(task.id, "complete");
                      }}
                    >
                      <Check size={12} />
                    </button>
                    <button
                      type="button"
                      className="island-preview-action danger"
                      disabled={busyId === task.id || pendingDeleteId !== null}
                      aria-label="删除"
                      title="删除"
                      onClick={(event) => {
                        event.stopPropagation();
                        void runPreviewAction(task.id, "archive");
                      }}
                    >
                      <Trash2 size={12} />
                    </button>
                  </div>
                );
              })
            )}
          </div>

          <div className="island-expanded-divider" />

          <div className="island-expanded-actions">
            <button
              type="button"
              className="island-action-btn"
              title="快速新增任务"
              onClick={(event) => {
                event.stopPropagation();
                void openQuickAdd();
              }}
            >
              <Plus size={14} />
            </button>
            <button
              type="button"
              className={`island-action-btn${isPinned ? " is-active" : ""}`}
              title={isPinned ? "取消固定展开" : "固定展开（保持展开不自动收起）"}
              onClick={(event) => {
                event.stopPropagation();
                postponePanelPreload();
                setIsPinned((v) => {
                  const next = !v;
                  if (next) {
                    cancelIslandResizeAnimation();
                    void expandIsland().then(() => ensureIslandAlwaysOnTop());
                  } else {
                    scheduleCollapse();
                  }
                  return next;
                });
              }}
            >
              <Pin size={14} />
            </button>
            <button
              type="button"
              className="island-action-btn"
              title="收起 TaskCap"
              onClick={(event) => {
                event.stopPropagation();
                setIsPinned(false);
                void collapseIsland();
              }}
            >
              <X size={14} />
            </button>
          </div>
        </div>
      </div>

      {/* 自定义提醒层 — 最高优先级，覆盖全部其他状态 */}
      <div className={`island-layer${showsCustomReminder ? " is-visible" : ""}`}>
        {customReminder ? (
          <button
            type="button"
            className={`island-reminder-btn is-${customReminder.type}`}
            onClick={dismissCustomReminder}
          >
            <div className="island-reminder-track">
              <span className={`island-reminder-scroll is-${customReminder.type}`}>
                {customReminder.type === "water" ? "💧 该喝水了" : "🏃 久坐提醒，起来动一动"}
                <span className="island-reminder-dismiss">　点击关闭</span>
              </span>
            </div>
          </button>
        ) : null}
      </div>
    </div>
  );
}
