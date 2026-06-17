import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { History, Pin, Settings, Undo2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { resolveAppGlassStyle } from "../../lib/appGlassStyle";
import { createPointerDragTracker } from "../../lib/windowDrag";
import { setWindowAlwaysOnTop } from "../../lib/windowZOrder";
import "../../styles/glass.css";
import "../../styles/panel.css";
import { PanelSnapshot, SearchTasksResult } from "./panelTypes";
import { PanelViewId } from "./panelViews";
import { HistoryPanelView } from "./HistoryPanelView";
import { SettingsPanelView } from "./SettingsPanelView";
import { TaskPanelView } from "./TaskPanelView";
import { useAppConfig } from "./useAppConfig";
import { usePanelSettings } from "./usePanelSettings";

const EMPTY_SNAPSHOT: PanelSnapshot = {
  menuBarTitle: "暂无当前任务",
  counts: { high: 0, medium: 0, low: 0, total: 0 },
  todayCount: 0,
  incomplete: [],
  completed: [],
  suggested: [],
  allTags: [],
  allTagSuggestions: [],
  allProjects: [],
  review: { completedToday: 0, postponedToday: 0, tomorrowCount: 0, focusMinutes: 0 },
  focusTask: null,
};

/** 任务面板 — M4：10 视图 + 搜索 + 详情 + 设置 */
export function PanelWindow() {
  const { settings, save } = usePanelSettings();
  const { config: appConfig, save: saveAppConfig } = useAppConfig();
  const [snapshot, setSnapshot] = useState<PanelSnapshot>(EMPTY_SNAPSHOT);
  const [viewMode, setViewMode] = useState<PanelViewId>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchTasksResult | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  /** 首次 get_panel_snapshot 完成前为 false，避免 EMPTY_SNAPSHOT 闪「暂无任务」 */
  const [snapshotLoaded, setSnapshotLoaded] = useState(false);
  const [isPinned, setIsPinned] = useState(false);
  const [isPanelDragging, setIsPanelDragging] = useState(false);
  const [busyId] = useState<string | null>(null);
  const panelDrag = useRef(createPointerDragTracker());
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** 设置页切换「显示灵动岛」时 island.show() 会抢焦点，短暂跳过失焦隐藏 */
  const suppressBlurHideUntilRef = useRef(0);

  const uiGlassActive = appConfig.capsuleTransparencyPercent > 0;
  const panelGlassStyle = useMemo(
    () => resolveAppGlassStyle(appConfig.capsuleTransparencyPercent, settings.darkGlassMode, true),
    [appConfig.capsuleTransparencyPercent, settings.darkGlassMode],
  );

  const refresh = useCallback(async () => {
    try {
      const data = await invoke<PanelSnapshot>("get_panel_snapshot");
      setSnapshot(data);
      return data;
    } finally {
      setSnapshotLoaded(true);
    }
  }, []);

  const handleShowCapsuleChange = useCallback((checked: boolean) => {
    // 仅打开时 island.show() 会抢焦点；关闭时不做失焦隐藏豁免
    if (checked) {
      hideTimer.current && clearTimeout(hideTimer.current);
      suppressBlurHideUntilRef.current = Date.now() + 600;
    }
    void saveAppConfig({ showCapsule: checked });
  }, [saveAppConfig]);

  useEffect(() => {
    refresh().catch(console.error);
    const initialRefreshTimer = window.setTimeout(() => {
      refresh().catch(console.error);
    }, 3000);
    const unlistenTasks = listen("tasks-changed", () => {
      refresh().catch(console.error);
    });
    const unlistenPanelRefresh = listen("panel-refresh-requested", () => {
      refresh().catch(console.error);
    });
    return () => {
      window.clearTimeout(initialRefreshTimer);
      void unlistenTasks.then((off) => off());
      void unlistenPanelRefresh.then((off) => off());
    };
  }, [refresh]);

  useEffect(() => {
    const secs = Math.min(600, Math.max(15, appConfig.panelRefreshIntervalSecs ?? 60));
    const timer = setInterval(() => {
      void refresh();
    }, secs * 1000);
    return () => clearInterval(timer);
  }, [refresh, appConfig.panelRefreshIntervalSecs]);

  // 面板加载/displayMode 变更时同步窗口宽度
  useEffect(() => {
    void invoke("apply_display_mode", { mode: appConfig.displayMode ?? "standard" }).catch(console.error);
  }, [appConfig.displayMode]);

  useEffect(() => {
    const trimmed = searchQuery.trim();
    if (!trimmed) {
      setSearchResults(null);
      return;
    }
    const timer = setTimeout(() => {
      invoke<SearchTasksResult>("search_tasks", { query: trimmed })
        .then(setSearchResults)
        .catch(console.error);
    }, 200);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  useEffect(() => {
    void setWindowAlwaysOnTop(isPinned);
  }, [isPinned]);

  useEffect(() => {
    const win = getCurrentWindow();
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      hideTimer.current && clearTimeout(hideTimer.current);
      if (focused && isPinned) {
        void setWindowAlwaysOnTop(true);
      }
      if (focused || isPinned || isPanelDragging) return;
      if (Date.now() < suppressBlurHideUntilRef.current) return;
      // 失焦隐藏前先记住当前位置，下次打开恢复
      hideTimer.current = setTimeout(() => {
        void (async () => {
          try {
            const pos = await win.outerPosition();
            await invoke("save_panel_position", { x: pos.x, y: pos.y });
          } catch (error) {
            console.error(error);
          }
          await win.hide();
        })();
      }, 160);
    });
    return () => {
      hideTimer.current && clearTimeout(hideTimer.current);
      void unlisten.then((off) => off());
    };
  }, [isPinned, isPanelDragging]);

  const handlePanelTitlePointerDown = (event: React.PointerEvent) => {
    if (event.target instanceof Element && event.target.closest("button, input, textarea, select")) return;
    if (!panelDrag.current.onPointerDown(event)) return;
    setIsPanelDragging(true);
  };

  const handlePanelTitlePointerUp = () => {
    const dragged = panelDrag.current.onPointerUp();
    setIsPanelDragging(false);
    // 拖动结束实时保存位置，下次打开（含失焦隐藏后再开）记住该位置
    if (dragged) {
      void (async () => {
        try {
          const pos = await getCurrentWindow().outerPosition();
          await invoke("save_panel_position", { x: pos.x, y: pos.y });
        } catch (error) {
          console.error(error);
        }
      })();
    }
  };

  useEffect(() => {
    document.documentElement.classList.add("panel-root");
    return () => document.documentElement.classList.remove("panel-root");
  }, []);

  return (
    <div
      className={`panel-shell panel-surface${settings.darkGlassMode ? " panel-surface-dark" : ""}${uiGlassActive ? " ui-glass-active" : ""}`}
      style={uiGlassActive ? panelGlassStyle : undefined}
    >
      <header
        className="panel-header panel-header-bar panel-drag-handle"
        data-tauri-drag-region
        title="拖动面板"
        onPointerDown={handlePanelTitlePointerDown}
        onPointerMove={(e) => panelDrag.current.onPointerMove(e)}
        onPointerUp={handlePanelTitlePointerUp}
        onPointerCancel={handlePanelTitlePointerUp}
      >
        <div
          className="panel-title-group"
          data-tauri-drag-region
        >
          <img src="/app-icon.png" alt="" className="panel-icon" draggable={false} />
          <div>
            <h1>TaskCap</h1>
            <p className="panel-subtitle">
              <span className="panel-count-pill">
                今天 <span className="panel-count-badge">{snapshot.todayCount}</span>
              </span>
              <span className="panel-count-pill">
                全部 <span className="panel-count-badge">{snapshot.counts.total}</span>
              </span>
            </p>
          </div>
        </div>
        <div className="panel-actions" data-tauri-drag-region>
          <button
            type="button"
            className={`icon-btn${isPinned ? " is-active" : ""}`}
            aria-label="固定面板"
            title={isPinned ? "取消置顶：恢复普通层级并失焦隐藏" : "置顶：保持最前且失焦不隐藏"}
            onClick={() => setIsPinned((v) => !v)}
          >
            <Pin size={16} />
          </button>
          <button
            type="button"
            className={`icon-btn${showHistory ? " is-active" : ""}`}
            aria-label="历史"
            title="已完成历史"
            onClick={() => {
              setShowHistory((v) => !v);
              setShowSettings(false);
            }}
          >
            <History size={16} />
          </button>
          <button
            type="button"
            className={`icon-btn${showSettings ? " is-active" : ""}`}
            aria-label="设置"
            title={showSettings ? "已在设置页" : "打开设置"}
            onClick={() => {
              setShowSettings((open) => !open);
              setShowHistory(false);
            }}
          >
            <Settings size={16} />
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label={showSettings || showHistory ? "返回任务" : "关闭"}
            title={showSettings || showHistory ? "返回任务列表" : "隐藏面板"}
            onClick={() => {
              if (showSettings) {
                setShowSettings(false);
                return;
              }
              if (showHistory) {
                setShowHistory(false);
                return;
              }
              void getCurrentWindow().hide();
            }}
          >
            {showSettings || showHistory ? (
              <Undo2 size={16} />
            ) : (
              <X size={16} />
            )}
          </button>
        </div>
      </header>

      <section className="panel-body panel-body-scroll">
        {showSettings ? (
          <SettingsPanelView
            settings={settings}
            appConfig={appConfig}
            onSave={save}
            onSaveAppConfig={saveAppConfig}
            onShowCapsuleChange={handleShowCapsuleChange}
            onRefresh={() => { refresh().catch(console.error); }}
          />
        ) : null}
        {/* 保持任务视图挂载，避免进入设置后专注计时被卸载重置 */}
        {showHistory ? <HistoryPanelView /> : null}
        {!showSettings && !showHistory && !snapshotLoaded ? (
          <div className="panel-loading" role="status" aria-live="polite" aria-busy="true">
            <img
              src="/panel-loading.png"
              alt=""
              className="panel-loading-gif"
              draggable={false}
            />
            <p className="panel-loading-text">数据加载中...</p>
          </div>
        ) : null}
        <div
          className="panel-view-keepalive"
          hidden={showSettings || showHistory || !snapshotLoaded}
        >
          <TaskPanelView
            snapshot={snapshot}
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
            searchResults={searchResults}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
            busyId={busyId}
            onRefresh={() => {
              refresh().catch(console.error);
            }}
          />
        </div>
      </section>
    </div>
  );
}
