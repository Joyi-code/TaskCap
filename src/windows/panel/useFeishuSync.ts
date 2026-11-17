import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

/** 与 Rust feishu::FeishuState 对应（不包含 app_secret） */
export type FeishuSyncState = {
  enabled: boolean;
  appId: string;
  appToken: string;
  tableId: string;
  autoSyncMinutes: number;
  hasSecret: boolean;
  lastSyncAt: string | null;
  lastSyncSummary: string | null;
  syncing: boolean;
  lastError: string | null;
};

export type FeishuSyncReport = {
  total: number;
  created: number;
  updated: number;
  pulled: number;
  skipped: number;
  durationMs: number;
};

export type FeishuConfigPatch = {
  enabled?: boolean;
  appId?: string;
  appSecret?: string;
  appToken?: string;
  tableId?: string;
  autoSyncMinutes?: number;
};

export type FeishuTableOutcome = {
  state: FeishuSyncState;
  message: string;
};

export type FeishuConnectionInfo = {
  appToken: string;
  tableId: string;
  tableName: string | null;
  fieldCount: number;
  message: string;
};

/** 连接自检结果：unknown=还没测过 */
export type FeishuConnection = "unknown" | "ok" | "fail";

function formatSyncTime(raw: string | null): string {
  if (!raw) return "尚未同步";
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) return raw;
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

/** 飞书同步状态：读配置、保存配置、手动同步、连接自检 */
export function useFeishuSync() {
  const [state, setState] = useState<FeishuSyncState | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [connection, setConnection] = useState<FeishuConnection>("unknown");
  const [lastCheck, setLastCheck] = useState<FeishuConnectionInfo | null>(null);

  const refresh = useCallback(async () => {
    try {
      setState(await invoke<FeishuSyncState>("feishu_get_state"));
    } catch (error) {
      setMessage(`读取飞书配置失败：${String(error)}`);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const save = useCallback(async (patch: FeishuConfigPatch) => {
    const next = await invoke<FeishuSyncState>("feishu_save_config", { patch });
    setState(next);
    return next;
  }, []);

  const syncNow = useCallback(async () => {
    setBusy(true);
    setMessage("同步中…");
    try {
      const report = await invoke<FeishuSyncReport>("feishu_sync_now");
      setConnection("ok");
      setMessage(
        `同步完成：新增 ${report.created}，更新 ${report.updated}，回写完成 ${report.pulled}，跳过 ${report.skipped}，耗时 ${report.durationMs} ms`,
      );
    } catch (error) {
      setConnection("fail");
      setMessage(`同步失败：${String(error)}`);
    } finally {
      setBusy(false);
      void refresh();
    }
  }, [refresh]);

  /** 用 App Token 一键创建/复用「TaskCap任务」表 */
  const createTable = useCallback(
    async (appToken: string) => {
      setBusy(true);
      setMessage("正在创建/复用任务表…");
      try {
        const outcome = await invoke<FeishuTableOutcome>("feishu_create_table", { appToken });
        setState(outcome.state);
        setConnection("ok");
        setMessage(outcome.message);
        // 建表成功后立刻做一次只读自检，把表名和字段数展示出来
        try {
          setLastCheck(await invoke<FeishuConnectionInfo>("feishu_test_connection"));
        } catch {
          // 自检失败不影响建表结果
        }
        return outcome;
      } catch (error) {
        setConnection("fail");
        setMessage(`建表失败：${String(error)}`);
        throw error;
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  /** 连接自检：只读校验凭证、token 与目标表 */
  const testConnection = useCallback(async () => {
    setBusy(true);
    setMessage("正在检查连接…");
    try {
      const info = await invoke<FeishuConnectionInfo>("feishu_test_connection");
      setLastCheck(info);
      setConnection("ok");
      setMessage(info.message);
      return info;
    } catch (error) {
      setConnection("fail");
      setMessage(`连接失败：${String(error)}`);
      throw error;
    } finally {
      setBusy(false);
      void refresh();
    }
  }, [refresh]);

  return {
    state,
    busy,
    message,
    setMessage,
    connection,
    lastCheck,
    refresh,
    save,
    syncNow,
    createTable,
    testConnection,
    formatSyncTime,
  };
}
