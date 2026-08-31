import { Clock, Power, Square } from "lucide-react";
import { api, fmtDuration } from "../api";
import type { Snapshot } from "../types";
import { useI18n } from "../i18n";

export default function Running({ snap }: { snap: Snapshot }) {
  const { t } = useI18n();
  const running = snap.running;
  return (
    <div className="view">
      <div className="view-head">
        <h2 className="view-title">{t("Running", "正在运行")}</h2>
        <span className="muted">{t(`${running.length} processes`, `${running.length} 个进程`)}</span>
        {running.length > 0 && (
          <button className="btn danger" onClick={() => api.terminateAll()}>
            <Power size={14} /> {t("Terminate all", "全部终止")}
          </button>
        )}
      </div>

      {running.length === 0 && (
        <div className="empty">{t("No Mosaic-managed processes are running.", "当前没有 Mosaic 管理的进程在运行。")}</div>
      )}

      <div className="rows">
        {running.map((h) => (
          <div key={h.execId} className="row">
            <div className="row-main">
              <span className="dot info" />
              <span className="row-name">{h.nickname}</span>
              <span className={"tag" + (h.lifecycle === "resident" ? "" : " info")}>
                {h.lifecycle === "resident" ? t("Keep running", "保持运行") : t("Exit when complete", "处理完关闭")}
              </span>
              <button className="btn small danger" onClick={() => api.terminate(h.taskId)}>
                <Square size={13} /> {t("Terminate", "终止")}
              </button>
            </div>
            <div className="row-meta">
              <span>
                <Clock size={12} /> {fmtDuration(h.uptimeSecs)}
              </span>
              <span>PID {h.pid}</span>
              <span className="mono">{h.command}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
