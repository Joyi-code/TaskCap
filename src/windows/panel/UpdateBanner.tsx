import { Download, X } from "lucide-react";
import { useUpdater } from "./useUpdater";

/**
 * 主界面顶部的新版本提示条。
 *
 * 只在发现新版本、且用户没有忽略这个版本时出现；
 * 点「立即更新」后应用会下载安装包并静默安装，随后自动重启。
 * 任务数据在 %APPDATA%，升级不会动它们。
 */
export function UpdateBanner() {
  const { state, apply, skip } = useUpdater();
  const info = state?.available;
  if (!info || state?.skippedVersion === info.version) return null;

  const applying = Boolean(state?.applying);
  const percent =
    state && state.totalBytes > 0
      ? Math.min(100, Math.round((state.downloadedBytes * 100) / state.totalBytes))
      : null;

  return (
    <div className="panel-update-banner" role="status">
      <Download size={14} className="panel-update-icon" />
      <div className="panel-update-text">
        <b>
          {applying
            ? percent !== null
              ? `正在下载 ${percent}%`
              : "正在下载…"
            : `发现新版本 v${info.version}`}
        </b>
        <span className="panel-update-hint">
          {applying ? "下载完成后自动安装并重启" : "升级只替换程序，任务数据不受影响"}
        </span>
      </div>
      <div className="panel-update-actions">
        <button
          type="button"
          className="panel-update-btn"
          disabled={applying}
          onClick={() => void apply()}
        >
          {applying ? "更新中" : "立即更新"}
        </button>
        {!applying ? (
          <button
            type="button"
            className="panel-update-skip"
            aria-label="稍后提醒"
            title="这个版本先不提示"
            onClick={() => void skip(info.version)}
          >
            <X size={13} />
          </button>
        ) : null}
      </div>
    </div>
  );
}
