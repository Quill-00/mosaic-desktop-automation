import { useMemo, useState } from "react";
import { Activity, ChevronDown, Clock, History } from "lucide-react";
import { relTime } from "../api";
import type { Execution, Snapshot } from "../types";

function statusText(e: Execution): string {
  switch (e.status) {
    case "running":
      return "运行中";
    case "ok":
      return "成功";
    case "failed":
      return "失败";
    case "timedOut":
      return "超时";
    case "killed":
      return "已终止";
    default:
      return e.status;
  }
}

function dotClass(e: Execution): string {
  if (e.status === "ok") return "ok";
  if (e.status === "running") return "info";
  return "danger";
}

function fmtFuture(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s <= 0) return "即将";
  if (s < 60) return `${s} 秒后`;
  if (s < 3600) return `${Math.round(s / 60)} 分钟后`;
  if (s < 86400) return `${Math.round(s / 3600)} 小时后`;
  return `${Math.round(s / 86400)} 天后`;
}

export default function StatusBar({ snap }: { snap: Snapshot }) {
  const [open, setOpen] = useState(false);
  const running = snap.running;
  const lastFinished = useMemo(() => snap.executions.find((e) => e.finishedAt), [snap.executions]);
  const recent = snap.executions.slice(0, 10);

  const upcoming = useMemo(() => {
    const now = Date.now();
    const lastStart: Record<string, number> = {};
    for (const e of snap.executions) {
      const t = new Date(e.startedAt).getTime();
      if (!isNaN(t) && (lastStart[e.taskId] === undefined || t > lastStart[e.taskId])) {
        lastStart[e.taskId] = t;
      }
    }
    const list: { name: string; at: number }[] = [];
    for (const task of snap.tasks) {
      if (!task.enabled) continue;
      const tr = task.trigger;
      if (tr.kind === "interval" && tr.everySecs) {
        const base = lastStart[task.id] ?? now;
        list.push({ name: task.nickname, at: base + tr.everySecs * 1000 });
      } else if (tr.kind === "daily" && tr.at) {
        const [h, m] = tr.at.split(":").map(Number);
        const d = new Date();
        d.setHours(h || 0, m || 0, 0, 0);
        if (d.getTime() <= now) d.setDate(d.getDate() + 1);
        list.push({ name: task.nickname, at: d.getTime() });
      } else if (tr.kind === "weekly" && tr.at && tr.days && tr.days.length) {
        const [h, m] = tr.at.split(":").map(Number);
        let best = Infinity;
        for (let add = 0; add < 8; add++) {
          const d = new Date();
          d.setDate(d.getDate() + add);
          d.setHours(h || 0, m || 0, 0, 0);
          if (tr.days.includes(d.getDay()) && d.getTime() > now) best = Math.min(best, d.getTime());
        }
        if (best < Infinity) list.push({ name: task.nickname, at: best });
      } else if (tr.kind === "monthly" && tr.at && tr.day) {
        const [h, m] = tr.at.split(":").map(Number);
        const day = tr.day;
        const mk = (y: number, mo: number) => {
          const dim = new Date(y, mo + 1, 0).getDate();
          return new Date(y, mo, Math.min(day, dim), h || 0, m || 0, 0, 0).getTime();
        };
        const dt = new Date();
        let t = mk(dt.getFullYear(), dt.getMonth());
        if (t <= now) t = mk(dt.getMonth() === 11 ? dt.getFullYear() + 1 : dt.getFullYear(), (dt.getMonth() + 1) % 12);
        list.push({ name: task.nickname, at: t });
      }
    }
    return list.sort((a, b) => a.at - b.at).slice(0, 8);
  }, [snap.tasks, snap.executions]);

  return (
    <div className="statusbar">
      <button className="statusbar-head" onClick={() => setOpen(!open)}>
        {running.length > 0 ? (
          <>
            <Activity size={15} className="spin" />
            <span>正在运行 {running.length} 个：{running.map((r) => r.nickname).join("、")}</span>
          </>
        ) : lastFinished ? (
          <>
            <Clock size={15} className="muted" />
            <span>
              上次运行 {lastFinished.nickname} · {relTime(lastFinished.finishedAt)} · {statusText(lastFinished)}
            </span>
          </>
        ) : (
          <>
            <Clock size={15} className="muted" />
            <span>还没有运行记录</span>
          </>
        )}
        <ChevronDown size={14} className={"muted chevron" + (open ? " open" : "")} style={{ marginLeft: "auto" }} />
      </button>

      {open && (
        <div className="statusbar-panel">
          <div className="sb-col">
            <div className="sb-title">
              <History size={13} /> 运行记录
            </div>
            {recent.length === 0 && <div className="muted small">暂无</div>}
            {recent.map((e) => (
              <div key={e.id} className="sb-row">
                <span className={"dot " + dotClass(e)} />
                <span className="sb-name">{e.nickname}</span>
                <span className="muted small">
                  {statusText(e)} · {relTime(e.finishedAt || e.startedAt)}
                </span>
              </div>
            ))}
          </div>
          <div className="sb-col">
            <div className="sb-title">
              <Clock size={13} /> 即将运行
            </div>
            {upcoming.length === 0 && <div className="muted small">没有排程中的自动化</div>}
            {upcoming.map((u, i) => (
              <div key={i} className="sb-row">
                <span className="sb-name">{u.name}</span>
                <span className="muted small">{fmtFuture(u.at - Date.now())}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
