import { Clock, Power, Square } from "lucide-react";
import { api, fmtDuration } from "../api";
import type { Snapshot } from "../types";

export default function Running({ snap }: { snap: Snapshot }) {
  const running = snap.running;
  return (
    <div className="view">
      <div className="view-head">
        <h2 className="view-title">正在运行</h2>
        <span className="muted">{running.length} 个进程</span>
        {running.length > 0 && (
          <button className="btn danger" onClick={() => api.terminateAll()}>
            <Power size={14} /> 全部终止
          </button>
        )}
      </div>

      {running.length === 0 && (
        <div className="empty">当前没有 Mosaic 管理的进程在运行。</div>
      )}

      <div className="rows">
        {running.map((h) => (
          <div key={h.execId} className="row">
            <div className="row-main">
              <span className="dot info" />
              <span className="row-name">{h.nickname}</span>
              <span className={"tag" + (h.lifecycle === "resident" ? "" : " info")}>
                {h.lifecycle === "resident" ? "保持运行" : "处理完关闭"}
              </span>
              <button className="btn small danger" onClick={() => api.terminate(h.taskId)}>
                <Square size={13} /> 终止
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
