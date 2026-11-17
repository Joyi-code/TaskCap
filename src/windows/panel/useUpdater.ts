import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

export type UpdateInfo = {
  version: string;
  notes: string;
  assetName: string;
  downloadUrl: string;
  size: number;
  sha256: string;
  isInstaller: boolean;
};

export type UpdateSnapshot = {
  currentVersion: string;
  checking: boolean;
  applying: boolean;
  available: UpdateInfo | null;
  skippedVersion: string | null;
  downloadedBytes: number;
  totalBytes: number;
  lastCheckAt: string | null;
  lastError: string | null;
  lastApplyOk: boolean | null;
  lastApplyMessage: string | null;
};

/** 应用内更新：查版本、下载安装、忽略某版本 */
export function useUpdater() {
  const [state, setState] = useState<UpdateSnapshot | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void invoke<UpdateSnapshot>("update_get_state")
      .then(setState)
      .catch(console.error);
    void listen<UpdateSnapshot>("update-state", (event) => {
      setState(event.payload);
    }).then((dispose) => {
      unlisten = dispose;
    });
    return () => unlisten?.();
  }, []);

  const check = useCallback(async () => {
    setMessage("正在检查…");
    try {
      const next = await invoke<UpdateSnapshot>("update_check", { force: true });
      // 响应里可能还带着「检查中」；命令已返回说明这次检查结束了
      setState({ ...next, checking: false });
      setMessage(
        next.available
          ? `发现新版本 v${next.available.version}`
          : next.lastError
            ? `检查失败：${next.lastError}`
            : "已是最新版本",
      );
      return next;
    } catch (error) {
      setMessage(`检查失败：${String(error)}`);
      throw error;
    }
  }, []);

  const apply = useCallback(async () => {
    try {
      await invoke("update_apply");
      setMessage("正在下载并安装，完成后会自动重启");
    } catch (error) {
      setMessage(`更新失败：${String(error)}`);
    }
  }, []);

  const skip = useCallback(async (version: string) => {
    try {
      setState(await invoke<UpdateSnapshot>("update_skip", { version }));
    } catch (error) {
      setMessage(`忽略失败：${String(error)}`);
    }
  }, []);

  return { state, message, check, apply, skip };
}

export function formatUpdateTime(raw: string | null): string {
  if (!raw) return "从未检查";
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) return raw;
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
