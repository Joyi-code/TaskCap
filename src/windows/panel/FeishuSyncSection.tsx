import { Cloud } from "lucide-react";
import { useEffect, useState } from "react";
import { SettingSection } from "./SettingSection";
import { SettingToggle } from "./SettingToggle";
import { useFeishuSync } from "./useFeishuSync";

/**
 * 飞书同步设置分区（可选功能）。
 *
 * 默认关闭：不用飞书的用户完全离线运行，不会发起任何网络请求。
 * 开启后填应用凭证和表格信息即可接入，数据落到「TaskCap任务」表：
 * 本地任务推送过去，飞书里勾「已完成」再回写本地。
 */
export function FeishuSyncSection() {
  const {
    state,
    busy,
    message,
    connection,
    lastCheck,
    setMessage,
    save,
    syncNow,
    createTable,
    testConnection,
    formatSyncTime,
  } = useFeishuSync();
  const [showConfig, setShowConfig] = useState(false);
  const [draft, setDraft] = useState({
    link: "",
    appId: "",
    appSecret: "",
    appToken: "",
    tableId: "",
  });

  const configured = Boolean(
    state?.appId && state?.hasSecret && state?.appToken && state?.tableId,
  );

  useEffect(() => {
    if (!state) return;
    setDraft((prev) => ({
      ...prev,
      appId: state.appId,
      appToken: state.appToken,
      tableId: state.tableId,
    }));
    // 开了同步但还没配好，直接展开配置区
    if (state.enabled && !configured) setShowConfig(true);
  }, [state, configured]);

  /** 从多维表格链接里取 App Token 与 Table ID（/base/ 与 /wiki/ 两种形态都支持） */
  function handleParseLink() {
    const raw = draft.link.trim();
    const appMatch = raw.match(/\/(?:base|wiki)\/([A-Za-z0-9]+)/);
    if (!appMatch) {
      setMessage("没识别到表格链接，请粘贴包含 /base/ 或 /wiki/ 的完整链接");
      return;
    }
    const tableMatch = raw.match(/[?&]table=(tbl[A-Za-z0-9]+)/);
    setDraft((prev) => ({
      ...prev,
      appToken: appMatch[1],
      tableId: tableMatch ? tableMatch[1] : prev.tableId,
    }));
    setMessage(
      tableMatch
        ? `已解析：App Token ${appMatch[1]}，Table ID ${tableMatch[1]}`
        : `已解析 App Token ${appMatch[1]}，未包含 Table ID，可点「创建任务表」生成`,
    );
  }

  async function persistConfig() {
    try {
      await save({
        appId: draft.appId,
        appToken: draft.appToken,
        tableId: draft.tableId,
        // 留空表示沿用已保存的密钥
        ...(draft.appSecret.trim() ? { appSecret: draft.appSecret.trim() } : {}),
      });
      setDraft((prev) => ({ ...prev, appSecret: "" }));
      setShowConfig(false);
    } catch (error) {
      console.error(error);
    }
  }

  async function handleCreateTable() {
    try {
      await createTable(draft.appToken);
      setShowConfig(false);
    } catch {
      // 错误已通过 message 展示
    }
  }

  const status = (() => {
    if (!state) return { kind: "off", label: "读取中…" };
    if (!state.enabled) return { kind: "off", label: "未启用（离线运行）" };
    if (!configured) return { kind: "pending", label: "未连接：待填配置" };
    if (connection === "fail" || state.lastError) return { kind: "error", label: "连接异常" };
    if (connection === "ok" || state.lastSyncAt) return { kind: "ok", label: "已连接" };
    return { kind: "pending", label: "已配置，待首次同步" };
  })();

  return (
    <SettingSection icon={<Cloud size={15} />} title="飞书同步（可选）">
      <SettingToggle
        label="启用飞书同步"
        checked={state?.enabled ?? false}
        onChange={(checked) => void save({ enabled: checked })}
      />
      <div className="panel-feishu-status-row">
        <span className={`panel-feishu-status is-${status.kind}`}>
          <i aria-hidden />
          {status.label}
        </span>
        {configured ? (
          <span className="panel-feishu-meta">
            上次同步：{formatSyncTime(state?.lastSyncAt ?? null)}
          </span>
        ) : null}
      </div>
      <p className="panel-setting-desc muted">
        关闭时 TaskCap 完全离线，不联网、不上传。开启后任务会同步到飞书多维表格「TaskCap任务」，
        在飞书里勾「已完成」也会回写本地。
      </p>

      {state?.enabled && !configured ? (
        <p className="panel-feishu-hint">
          还没配好：请填写 App ID、App Secret、App Token，然后点「创建任务表」。
        </p>
      ) : null}

      <div className="panel-capsule-group">
        <button
          type="button"
          className="panel-capsule-btn panel-capsule-icon-btn"
          disabled={busy || state?.syncing || !state?.enabled || !configured}
          onClick={() => void syncNow()}
        >
          <span className="panel-capsule-arrow">↻</span>
          <span>{busy || state?.syncing ? "同步中…" : "立即同步"}</span>
        </button>
        <button
          type="button"
          className="panel-capsule-btn panel-capsule-icon-btn"
          disabled={busy || !state?.hasSecret || !state?.appToken}
          onClick={() => void testConnection().catch(() => undefined)}
        >
          <span>测试连接</span>
        </button>
        <button
          type="button"
          className="panel-capsule-btn panel-capsule-icon-btn"
          onClick={() => setShowConfig((prev) => !prev)}
        >
          <span>{showConfig ? "收起配置" : "连接配置"}</span>
        </button>
      </div>

      {status.kind === "ok" ? (
        <div className="panel-feishu-detail">
          <div className="panel-feishu-detail-row">
            <span>数据表</span>
            <b>{lastCheck?.tableName ?? "TaskCap任务"}</b>
          </div>
          <div className="panel-feishu-detail-row">
            <span>Table ID</span>
            <b className="panel-feishu-mono">{lastCheck?.tableId ?? state?.tableId ?? "—"}</b>
          </div>
          <div className="panel-feishu-detail-row">
            <span>字段数</span>
            <b>{lastCheck ? `${lastCheck.fieldCount} 个` : "点「测试连接」查看"}</b>
          </div>
          <div className="panel-feishu-detail-row">
            <span>上次同步</span>
            <b>{formatSyncTime(state?.lastSyncAt ?? null)}</b>
          </div>
          {state?.lastSyncSummary ? (
            <p className="panel-feishu-meta muted">{state.lastSyncSummary}</p>
          ) : null}
        </div>
      ) : null}
      {state?.lastError ? <p className="panel-feishu-error">错误：{state.lastError}</p> : null}
      {message ? <p className="panel-setting-desc">{message}</p> : null}

      {showConfig ? (
        <div className="panel-feishu-config">
          <p className="panel-setting-desc muted">
            1. 开放平台建自建应用，开通「多维表格」读写权限，发布审核通过；把应用加为目标多维表格的「可编辑」协作者。
          </p>
          <p className="panel-setting-desc muted">
            2. App Token 取表格链接 <b>/base/</b> 或 <b>/wiki/</b> 后那段；Table ID 取 <b>?table=</b> 后那段，
            留空可点下方「创建任务表」自动生成。知识库链接会自动换算成真实表格 token。
          </p>
          <p className="panel-setting-desc muted">
            3. 凭证只存本机，关闭开关后不再联网。
          </p>
          <label className="panel-setting-text-row">
            <span>粘贴链接</span>
            <input
              className="panel-setting-text-input"
              value={draft.link}
              placeholder="从飞书复制的多维表格链接"
              onChange={(event) => setDraft((prev) => ({ ...prev, link: event.target.value }))}
            />
          </label>
          <div className="panel-capsule-group">
            <button
              type="button"
              className="panel-capsule-btn panel-capsule-icon-btn"
              disabled={!draft.link.trim()}
              onClick={handleParseLink}
            >
              <span>从链接解析</span>
            </button>
          </div>
          <label className="panel-setting-text-row">
            <span>App ID</span>
            <input
              className="panel-setting-text-input"
              value={draft.appId}
              placeholder="cli_ 开头，在「凭证与基础信息」里"
              onChange={(event) => setDraft((prev) => ({ ...prev, appId: event.target.value }))}
            />
          </label>
          <label className="panel-setting-text-row">
            <span>App Secret</span>
            <input
              className="panel-setting-text-input"
              type="password"
              value={draft.appSecret}
              placeholder={state?.hasSecret ? "已保存，留空不修改" : "同上页面里的应用密钥"}
              onChange={(event) => setDraft((prev) => ({ ...prev, appSecret: event.target.value }))}
            />
          </label>
          <label className="panel-setting-text-row">
            <span>App Token</span>
            <input
              className="panel-setting-text-input"
              value={draft.appToken}
              placeholder="链接 /base/ 或 /wiki/ 后面那段"
              onChange={(event) => setDraft((prev) => ({ ...prev, appToken: event.target.value }))}
            />
          </label>
          <label className="panel-setting-text-row">
            <span>Table ID</span>
            <input
              className="panel-setting-text-input"
              value={draft.tableId}
              placeholder="链接 ?table= 后面那段，可留空"
              onChange={(event) => setDraft((prev) => ({ ...prev, tableId: event.target.value }))}
            />
          </label>
          <label className="panel-setting-slider-row">
            <span>自动同步（{state?.autoSyncMinutes ?? 15} 分钟）</span>
            <input
              type="range"
              min={5}
              max={120}
              step={5}
              value={state?.autoSyncMinutes ?? 15}
              onChange={(event) =>
                void save({ autoSyncMinutes: Number(event.target.value) || 15 })
              }
            />
          </label>
          <div className="panel-capsule-group">
            <button
              type="button"
              className="panel-capsule-btn panel-capsule-icon-btn"
              onClick={() => void persistConfig()}
            >
              <span>保存配置</span>
            </button>
            <button
              type="button"
              className="panel-capsule-btn panel-capsule-icon-btn"
              disabled={busy || !draft.appToken.trim()}
              onClick={() => void handleCreateTable()}
            >
              <span>创建任务表</span>
            </button>
          </div>
        </div>
      ) : null}
    </SettingSection>
  );
}
