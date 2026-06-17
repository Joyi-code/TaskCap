import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ArrowUp } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useUiTransparency } from "../../lib/useUiTransparency";
import { PanelSnapshot } from "../panel/panelTypes";
import { QuickAddInput } from "../panel/QuickAddInput";
import { readPanelSettings, type PanelSettings } from "../panel/usePanelSettings";
import "../../styles/glass.css";
import "../../styles/panel.css";

const PANEL_SETTINGS_KEY = "taskcap.panel.settings";

declare global {
  interface Window {
    __taskcapFocusQuickAddInput?: () => void;
  }
}

/** 首次懒建 WebView 时窗口先获焦、input 尚未挂载，需持续重试聚焦 */
function focusQuickAddInput(input: HTMLInputElement | null, attempt = 0): boolean {
  const el =
    input ?? document.querySelector<HTMLInputElement>(".quickadd-input");
  if (!el) {
    if (attempt < 80) {
      window.setTimeout(
        () => focusQuickAddInput(null, attempt + 1),
        attempt < 6 ? 16 : 50,
      );
    }
    return false;
  }
  el.focus({ preventScroll: true });
  if (el.value.length === 0 && typeof el.select === "function") {
    el.select();
  }
  if (document.activeElement === el || attempt >= 80) {
    return document.activeElement === el;
  }
  window.setTimeout(() => focusQuickAddInput(el, attempt + 1), 50);
  return false;
}

/**
 * 全局快速新增窗口。
 *
 * 窗口高度固定（见 tauri.conf.json），默认只渲染顶部的矮卡片，其余区域透明。
 * Tab 唤起的符号菜单浮在卡片下方（白底可见），窗口本身足够高所以不会被裁切。
 * 失焦时自动隐藏，避免固定高度窗口的透明区域长期遮挡桌面点击。
 */
export function QuickAddWindow() {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const submitInFlightRef = useRef(false);
  const [text, setText] = useState("");
  const [knownTags, setKnownTags] = useState<string[]>([]);
  const [knownProjects, setKnownProjects] = useState<string[]>([]);
  const [panelSettings, setPanelSettings] = useState<PanelSettings>(readPanelSettings);
  const { glassStyle, isGlassActive } = useUiTransparency(panelSettings.darkGlassMode);
  const dark = panelSettings.darkGlassMode;

  const requestInputFocus = useCallback(() => {
    focusQuickAddInput(inputRef.current);
  }, []);

  useEffect(() => {
    window.__taskcapFocusQuickAddInput = requestInputFocus;
    return () => {
      delete window.__taskcapFocusQuickAddInput;
    };
  }, [requestInputFocus]);

  useEffect(() => {
    void invoke("quickadd_frontend_ready").catch(() => undefined);
  }, []);

  useEffect(() => {
    void invoke<PanelSnapshot>("get_panel_snapshot")
      .then((snapshot) => {
        setKnownTags(snapshot.allTagSuggestions);
        setKnownProjects(snapshot.allProjects);
      })
      .catch(() => {
        /* 快照失败时仍可使用 Tab 符号菜单 */
      });
  }, []);

  useEffect(() => {
    document.documentElement.classList.add("quickadd-root");
    return () => document.documentElement.classList.remove("quickadd-root");
  }, []);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key === PANEL_SETTINGS_KEY) {
        setPanelSettings(readPanelSettings());
      }
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const hide = useCallback(async () => {
    await getCurrentWindow().hide();
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      void hide();
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [hide]);

  // 点窗口外（失焦）自动隐藏，避免固定高度窗口的透明区域遮挡其他点击
  useEffect(() => {
    const win = getCurrentWindow();
    let hideTimer: ReturnType<typeof setTimeout> | null = null;
    const pending = win.onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        hideTimer = setTimeout(() => void win.hide(), 120);
      } else {
        if (hideTimer) {
          clearTimeout(hideTimer);
          hideTimer = null;
        }
        requestInputFocus();
      }
    });
    return () => {
      if (hideTimer) clearTimeout(hideTimer);
      void pending.then((off) => off());
    };
  }, [requestInputFocus]);

  useEffect(() => {
    const pending = Promise.all([
      listen("quickadd-opened", () => requestInputFocus()),
      listen("quickadd-focus-input", () => requestInputFocus()),
    ]);
    return () => {
      void pending.then((offs) => offs.forEach((off) => off()));
    };
  }, [requestInputFocus]);

  useEffect(() => {
    requestInputFocus();
  }, [requestInputFocus]);

  const submit = useCallback(async () => {
    if (submitInFlightRef.current) return;
    const trimmed = text.trim();
    if (!trimmed) return;
    submitInFlightRef.current = true;
    try {
      await invoke("quick_add_task", { text: trimmed });
      setText("");
      await hide();
    } finally {
      submitInFlightRef.current = false;
    }
  }, [text, hide]);

  return (
    <div
      className={`quickadd-shell panel-surface${dark ? " panel-surface-dark" : ""}${isGlassActive ? " ui-glass-active" : ""}`}
      style={isGlassActive ? glassStyle : undefined}
    >
      <div className="quickadd-input-row">
        <QuickAddInput
          inputRef={inputRef}
          autoFocus
          onInputMount={() => requestInputFocus()}
          className="quickadd-input"
          menuPlacement="below"
          placeholder="明天 10点 发周报 #工作 !高 /30m（Enter新增）"
          value={text}
          onChange={setText}
          knownTags={knownTags}
          knownProjects={knownProjects}
          onSubmit={() => void submit()}
          onRequestClose={() => void hide()}
        />
        <button
          type="button"
          className="icon-btn panel-add-btn quickadd-add-btn"
          aria-label="添加"
          onClick={() => void submit()}
        >
          <ArrowUp size={16} />
        </button>
      </div>
      <div className="quickadd-footer">
        <span className="quickadd-hint">示例：明天 10点 发周报 #工作 !高 /30m（Tab 唤起 # ! /）</span>
        <button type="button" className="quickadd-esc" onClick={() => void hide()}>
          Esc 取消
        </button>
      </div>
    </div>
  );
}
