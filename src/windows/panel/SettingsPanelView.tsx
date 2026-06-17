import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Bell, Database, Info, Keyboard, Layout, Monitor, Power, RefreshCw, Sparkles, Timer } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { MAX_UI_TRANSPARENCY_PERCENT } from "../../lib/appGlassStyle";
import { APP_VERSION } from "../../version";
import { SettingSection } from "./SettingSection";
import { SettingToggle } from "./SettingToggle";
import { AppConfig } from "./useAppConfig";
import { PanelSettings } from "./usePanelSettings";

const FOCUS_PRESETS = [15, 25, 45, 60] as const;
type ExportFormat = "json" | "markdown" | "csv";

function formatRefreshTime(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}年${pad(date.getMonth() + 1)}月${pad(date.getDate())}日 ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

type Props = {
  settings: PanelSettings;
  appConfig: AppConfig;
  onSave: (patch: Partial<PanelSettings>) => void;
  onSaveAppConfig: (patch: Partial<AppConfig>) => void | Promise<unknown>;
  onShowCapsuleChange: (checked: boolean) => void;
  onRefresh: () => void;
};

function formatShortcutDisplay(raw: string): string {
  return raw.split("+").join(" + ");
}

export function SettingsPanelView({
  settings,
  appConfig,
  onSave,
  onSaveAppConfig,
  onShowCapsuleChange,
  onRefresh,
}: Props) {
  const [exportFormat, setExportFormat] = useState<ExportFormat>("json");
  const [dataMessage, setDataMessage] = useState<string | null>(null);
  const [lastRefreshMessage, setLastRefreshMessage] = useState<string | null>(null);
  const [isRecording, setIsRecording] = useState(false);
  const [tempShortcut, setTempShortcut] = useState("");
  const recordingRef = useRef<HTMLDivElement>(null);

  const currentShortcut = appConfig.quickAddShortcut ?? "Ctrl+Alt+N";

  useEffect(() => {
    if (isRecording) recordingRef.current?.focus();
  }, [isRecording]);

  const handleShortcutKeyDown = useCallback((e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      setIsRecording(false);
      setTempShortcut("");
      return;
    }
    if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;
    const mods: string[] = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    const key = e.key.toUpperCase();
    setTempShortcut([...mods, key].join("+"));
  }, []);

  async function confirmShortcut() {
    if (!tempShortcut) { setIsRecording(false); return; }
    try {
      await invoke("update_shortcut", { shortcut: tempShortcut });
      void onSaveAppConfig({ quickAddShortcut: tempShortcut });
    } catch (err) {
      setDataMessage(`快捷键设置失败：${String(err)}`);
      setTimeout(() => setDataMessage(null), 3000);
    }
    setIsRecording(false);
    setTempShortcut("");
  }

  async function exportTasks() {
    const ext = exportFormat === "markdown" ? "md" : exportFormat;
    const path = await save({
      defaultPath: `taskcap-export.${ext}`,
      filters: [{ name: exportFormat.toUpperCase(), extensions: [ext] }],
    });
    if (!path) return;
    await invoke("export_tasks_to_path", { path, format: exportFormat });
    setDataMessage(`已导出：${path}`);
    setTimeout(() => setDataMessage(null), 3000);
  }

  async function importTasks() {
    const path = await open({
      multiple: false,
      filters: [
        { name: "JSON 备份", extensions: ["json"] },
        { name: "CSV 表格", extensions: ["csv"] },
      ],
    });
    if (!path || typeof path !== "string") return;
    const ext = path.split(/[/\\]/).pop()?.split(".").pop()?.toLowerCase();
    if (ext !== "json" && ext !== "csv") {
      setDataMessage("仅支持导入 JSON 或 CSV 文件");
      setTimeout(() => setDataMessage(null), 3000);
      return;
    }
    const count = await invoke<number>("import_tasks_from_path", { path });
    setDataMessage(`已导入 ${count} 条任务`);
    setTimeout(() => setDataMessage(null), 3000);
  }

  function handleRefresh() {
    onRefresh();
    setLastRefreshMessage(`已刷新 ${formatRefreshTime(new Date())}`);
  }

  return (
    <div className="panel-settings">
      {/* 显示 */}
      <SettingSection icon={<Monitor size={15} />} title="显示">
        <SettingToggle
          label="显示灵动岛"
          checked={appConfig.showCapsule}
          onChange={onShowCapsuleChange}
        />
        <SettingToggle
          label="菜单栏标题"
          checked={appConfig.showTitleInMenuBar}
          onChange={(checked) => void onSaveAppConfig({ showTitleInMenuBar: checked })}
        />
        <SettingToggle
          label="暗夜模式"
          checked={settings.darkGlassMode}
          onChange={(checked) => onSave({ darkGlassMode: checked })}
        />
      </SettingSection>

      {/* 悬浮岛 */}
      <SettingSection icon={<Sparkles size={15} />} title="灵动岛">
        <label className="panel-setting-slider-row">
          <span>顶部间距（{appConfig.islandOffsetY}px）</span>
          <input
            type="range"
            min={0}
            max={80}
            step={1}
            value={appConfig.islandOffsetY}
            onChange={(e) => void onSaveAppConfig({ islandOffsetY: Number(e.target.value) })}
          />
        </label>
        <label className="panel-setting-slider-row">
          <span>
            透明度（{Math.min(appConfig.capsuleTransparencyPercent, MAX_UI_TRANSPARENCY_PERCENT)}%）
          </span>
          <input
            type="range"
            min={0}
            max={MAX_UI_TRANSPARENCY_PERCENT}
            step={1}
            value={Math.min(appConfig.capsuleTransparencyPercent, MAX_UI_TRANSPARENCY_PERCENT)}
            onChange={(e) =>
              void onSaveAppConfig({ capsuleTransparencyPercent: Number(e.target.value) })
            }
          />
        </label>
      </SettingSection>

      {/* 开机自启 */}
      <SettingSection icon={<Power size={15} />} title="开机自启">
        <SettingToggle
          label="登录时自动启动"
          checked={appConfig.autostart}
          onChange={(checked) => void onSaveAppConfig({ autostart: checked })}
        />
      </SettingSection>

      {/* 提醒 */}
      <SettingSection icon={<Bell size={15} />} title="提醒">
        <SettingToggle
          label="💧 喝水提醒"
          checked={settings.waterReminderEnabled}
          onChange={(checked) => onSave({ waterReminderEnabled: checked })}
        />
        {settings.waterReminderEnabled && (
          <label className="panel-setting-slider-row">
            <span>间隔（{settings.waterReminderMinutes} 分钟）</span>
            <input
              type="range" min={1} max={120} step={1}
              value={settings.waterReminderMinutes}
              onChange={(e) => onSave({ waterReminderMinutes: Number(e.target.value) })}
            />
          </label>
        )}
        <SettingToggle
          label="🏃 久坐提醒"
          checked={settings.sittingReminderEnabled}
          onChange={(checked) => onSave({ sittingReminderEnabled: checked })}
        />
        {settings.sittingReminderEnabled && (
          <label className="panel-setting-slider-row">
            <span>间隔（{settings.sittingReminderMinutes} 分钟）</span>
            <input
              type="range" min={1} max={120} step={1}
              value={settings.sittingReminderMinutes}
              onChange={(e) => onSave({ sittingReminderMinutes: Number(e.target.value) })}
            />
          </label>
        )}
        <p className="panel-setting-desc muted">
          研究建议：喝水每 45 分钟、久坐每 60 分钟起来活动。提醒会在 TaskCap 滚动显示，点击关闭。
        </p>
      </SettingSection>

      {/* 显示模式 */}
      <SettingSection icon={<Layout size={15} />} title="显示模式">
        <div className="panel-capsule-group">
          <button
            type="button"
            className={`panel-capsule-btn${(appConfig.displayMode ?? "standard") === "standard" ? " is-active" : ""}`}
            onClick={() => void onSaveAppConfig({ displayMode: "standard" })}
          >
            标准（172px）
          </button>
          <button
            type="button"
            className={`panel-capsule-btn${appConfig.displayMode === "wide" ? " is-active" : ""}`}
            onClick={() => void onSaveAppConfig({ displayMode: "wide" })}
          >
            宽大（340px）
          </button>
        </div>
        <p className="panel-setting-desc muted">
          仅影响提醒弹出时的灵动岛宽度。标准与收起态同宽 172px，宽大与专注态同宽 340px。
        </p>
      </SettingSection>

      {/* 专注 */}
      <SettingSection icon={<Timer size={15} />} title="专注">
        <div className="panel-setting-label-row">
          <span>默认时长</span>
          <span className="panel-setting-slider-value">{settings.defaultFocusMinutes} 分钟</span>
        </div>
        <input
          className="panel-setting-slider-full"
          type="range"
          min={5}
          max={120}
          step={1}
          value={settings.defaultFocusMinutes}
          onChange={(e) => onSave({ defaultFocusMinutes: Number(e.target.value) || 25 })}
        />
        <div className="panel-setting-presets">
          {FOCUS_PRESETS.map((min) => (
            <button
              key={min}
              type="button"
              className={`panel-capsule-btn${settings.defaultFocusMinutes === min ? " is-active" : ""}`}
              onClick={() => onSave({ defaultFocusMinutes: min })}
            >
              {min} 分钟
            </button>
          ))}
        </div>
        <p className="panel-setting-desc">
          任务没有单独设置时，会使用这个默认时长。单个任务可在任务详情里修改。
        </p>
      </SettingSection>

      {/* 快捷键 */}
      <SettingSection icon={<Keyboard size={15} />} title="快捷键">
        <div className="panel-shortcut-row">
          <span className="panel-shortcut-label">快速新增</span>
          {isRecording ? (
            <div
              ref={recordingRef}
              className="panel-shortcut-recording"
              tabIndex={0}
              onKeyDown={handleShortcutKeyDown}
              onBlur={() => void confirmShortcut()}
            >
              {tempShortcut ? formatShortcutDisplay(tempShortcut) : "按下组合键…"}
            </div>
          ) : (
            <button
              type="button"
              className="panel-shortcut-keys panel-shortcut-editable"
              onClick={() => { setTempShortcut(""); setIsRecording(true); }}
              title="点击修改快捷键"
            >
              {formatShortcutDisplay(currentShortcut)}
            </button>
          )}
        </div>
        <p className="panel-setting-desc muted">
          点击快捷键可修改。支持 Ctrl / Alt / Shift + 字母/数字。按 Esc 取消。
        </p>
      </SettingSection>

      {/* 数据 */}
      <SettingSection icon={<RefreshCw size={15} />} title="数据">
        <SettingToggle
          label="后台预加载主面板"
          checked={appConfig.panelBackgroundPreload ?? true}
          onChange={(checked) => void onSaveAppConfig({ panelBackgroundPreload: checked })}
        />
        <p className="panel-setting-desc muted">
          开启后会在灵动岛就绪约 3 秒再后台加载主面板，避免与灵动岛抢资源；之后按下方间隔自动刷新。
        </p>
        <label className="panel-setting-slider-row">
          <span>自动刷新（{appConfig.panelRefreshIntervalSecs ?? 60} 秒）</span>
          <input
            type="range"
            min={15}
            max={300}
            step={15}
            value={appConfig.panelRefreshIntervalSecs ?? 60}
            onChange={(e) =>
              void onSaveAppConfig({ panelRefreshIntervalSecs: Number(e.target.value) })
            }
          />
        </label>
        <p className="panel-setting-desc muted">
          主面板 WebView 已挂载时生效（预加载开启，或您至少打开过一次主面板）。
        </p>
      </SettingSection>

      {/* 操作 */}
      <SettingSection icon={<Database size={15} />} title="操作">
        <div className="panel-setting-label-row panel-operation-format-label">
          <span>
            导出格式
            <span className="panel-setting-inline-note">（注:暂不支持markdown导入）</span>
          </span>
        </div>
        <div className="panel-capsule-group">
          {(["json", "markdown", "csv"] as ExportFormat[]).map((fmt) => (
            <button
              key={fmt}
              type="button"
              className={`panel-capsule-btn${exportFormat === fmt ? " is-active" : ""}`}
              onClick={() => setExportFormat(fmt)}
            >
              {fmt === "json" ? "JSON" : fmt === "markdown" ? "Markdown" : "CSV"}
            </button>
          ))}
        </div>
        <div className="panel-capsule-group">
          <button type="button" className="panel-capsule-btn panel-capsule-icon-btn" onClick={handleRefresh}>
            <RefreshCw size={13} />
            <span>刷新数据</span>
          </button>
          <button type="button" className="panel-capsule-btn panel-capsule-icon-btn" onClick={() => void exportTasks()}>
            <span className="panel-capsule-arrow">↑</span>
            <span>导出</span>
          </button>
          <button type="button" className="panel-capsule-btn panel-capsule-icon-btn" onClick={() => void importTasks()}>
            <span className="panel-capsule-arrow">↓</span>
            <span>导入</span>
          </button>
        </div>
        {lastRefreshMessage ? <p className="panel-setting-desc panel-refresh-timestamp">{lastRefreshMessage}</p> : null}
        {dataMessage ? <p className="panel-setting-desc">{dataMessage}</p> : null}
      </SettingSection>

      {/* 关于 */}
      <SettingSection icon={<Info size={15} />} title="关于">
        <div className="panel-version-row">
          <span>当前版本</span>
          <strong>{APP_VERSION}</strong>
        </div>
      </SettingSection>
    </div>
  );
}
