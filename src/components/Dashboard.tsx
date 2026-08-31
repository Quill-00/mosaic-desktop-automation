import { useMemo, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { AlertTriangle, Bell, Check, ChevronRight, GripVertical, Pencil, Plus, X } from "lucide-react";
import { api, relTime } from "../api";
import type { Snapshot, Task } from "../types";
import CardView from "./Cards";
import StatusBar from "./StatusBar";
import TaskDetail from "./TaskDetail";
import { useI18n } from "../i18n";

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

export default function Dashboard({ snap, onNavigate }: { snap: Snapshot; onNavigate?: (v: string) => void }) {
  const { locale, t: tr } = useI18n();
  const [openTask, setOpenTask] = useState<Task | null>(null);
  const [editing, setEditing] = useState(false);
  const [localIds, setLocalIds] = useState<string[] | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const [sizeOverride, setSizeOverride] = useState<{ id: string; col: number; row: number } | null>(null);

  const unread = snap.notifications.filter((n) => !n.read).length;

  const boardTasks = useMemo(
    () => snap.tasks.filter((t) => t.active && t.onDashboard !== false).sort((a, b) => (a.order ?? 0) - (b.order ?? 0)),
    [snap.tasks],
  );

  const currentIds = editing && localIds ? localIds : boardTasks.map((t) => t.id);
  const byId = (id: string) => snap.tasks.find((t) => t.id === id);
  const displayed = currentIds.map(byId).filter((t): t is Task => !!t);
  const offBoard = snap.tasks.filter((t) => t.active && !currentIds.includes(t.id));

  function startEdit() {
    setLocalIds(boardTasks.map((t) => t.id));
    setEditing(true);
  }
  function stopEdit() {
    setEditing(false);
    setLocalIds(null);
  }
  function commit(next: string[]) {
    setLocalIds(next);
    api.setDashboard(next);
  }
  function reorderTo(targetId: string) {
    if (!dragId || dragId === targetId) return;
    const arr = currentIds.filter((x) => x !== dragId);
    const idx = arr.indexOf(targetId);
    arr.splice(idx < 0 ? arr.length : idx, 0, dragId);
    commit(arr);
  }
  const removeFromBoard = (id: string) => commit(currentIds.filter((x) => x !== id));
  const addToBoard = (id: string) => commit([...currentIds, id]);

  // Drag a border to resize, snapping to whole grid steps on one axis — never a
  // free-pixel resize that warps the layout.
  function startResize(e: ReactPointerEvent, t: Task, axis: "x" | "y") {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startY = e.clientY;
    const startCol = t.colSpan ?? 1;
    const startRow = t.rowSpan ?? 1;
    const STEP_X = 130;
    const STEP_Y = 120;
    const move = (ev: PointerEvent) => {
      if (axis === "x") {
        const col = clamp(startCol + Math.round((ev.clientX - startX) / STEP_X), 1, 3);
        setSizeOverride({ id: t.id, col, row: startRow });
      } else {
        const row = clamp(startRow + Math.round((ev.clientY - startY) / STEP_Y), 1, 2);
        setSizeOverride({ id: t.id, col: startCol, row });
      }
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      setSizeOverride((cur) => {
        if (cur && cur.id === t.id) api.setModuleSpan(t.id, cur.col, cur.row);
        return null;
      });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  return (
    <div className="view">
      <StatusBar snap={snap} />

      {snap.notifications.length > 0 && (
        <section className="dash-notif">
          <div className="dash-notif-head">
            <Bell size={14} />
            <span>{tr("Notifications", "通知")}</span>
            {unread > 0 && <span className="pill">{tr(`${unread} unread`, `${unread} 未读`)}</span>}
            <span style={{ flex: 1 }} />
            {unread > 0 && (
              <button className="btn small" onClick={() => api.markAllRead()}>
                {tr("Mark all read", "全部已读")}
              </button>
            )}
            <button className="btn small" onClick={() => onNavigate?.("notifications")}>
              {tr("View all", "查看全部")}
            </button>
          </div>
          {snap.notifications.slice(0, 4).map((n) => (
            <div key={n.id} className={"dn-row" + (n.read ? " read" : "")}>
              {n.level === "danger" ? (
                <AlertTriangle size={14} className="danger-ic" />
              ) : (
                <Bell size={14} className="muted" />
              )}
              <span className="dn-title">{n.title}</span>
              <span className="muted small">{relTime(n.at, locale)}</span>
            </div>
          ))}
        </section>
      )}

      <div className="view-head">
        <h2 className="view-title">{tr("Dashboard", "仪表盘")}</h2>
        <span className="muted">{editing ? tr("Drag cards to reorder. Drag the right or bottom edge to resize.", "拖动卡片排序，拖右/下边框改大小") : ""}</span>
        {snap.tasks.length > 0 && (
          <button className="btn" onClick={() => (editing ? stopEdit() : startEdit())}>
            {editing ? (
              <>
                <Check size={14} /> {tr("Done", "完成")}
              </>
            ) : (
              <>
                <Pencil size={14} /> {tr("Edit layout", "编辑布局")}
              </>
            )}
          </button>
        )}
      </div>

      <div className="grid">
        {displayed.map((t) => {
          const r = snap.results[t.id];
          const disabled = !t.enabled;
          const col = sizeOverride && sizeOverride.id === t.id ? sizeOverride.col : t.colSpan ?? 1;
          const row = sizeOverride && sizeOverride.id === t.id ? sizeOverride.row : t.rowSpan ?? 1;
          return (
            <div
              key={t.id}
              className={"module" + (editing ? " editing" : "") + (disabled ? " disabled" : "")}
              style={{ gridColumn: `span ${col}`, gridRow: `span ${row}` }}
              onDragOver={editing ? (e) => e.preventDefault() : undefined}
              onDrop={editing ? () => reorderTo(t.id) : undefined}
              onClick={() => !editing && setOpenTask(t)}
            >
              <div className="module-head">
                {editing && (
                  <span className="drag-grip" draggable onDragStart={() => setDragId(t.id)} aria-label={tr("Drag to reorder", "拖动排序")}>
                    <GripVertical size={14} />
                  </span>
                )}
                <span className="module-name">{t.nickname}</span>
                {disabled && <span className="tag">{tr("Disabled", "已停用")}</span>}
                {r?.summary?.count != null && <span className="pill">{r.summary.count}</span>}
                <span style={{ flex: 1 }} />
                {editing ? (
                  <button className="icon-btn" onClick={() => removeFromBoard(t.id)} aria-label={tr("Remove", "移除")}>
                    <X size={14} />
                  </button>
                ) : (
                  <ChevronRight size={15} className="muted" />
                )}
              </div>
              <div className="module-body">
                {r?.card ? (
                  <CardView card={r.card} />
                ) : (
                  <div className="muted small">{tr("No data yet", "尚无数据")}{t.trigger.kind === "manual" ? "" : tr(" · waiting to run…", "，等待运行…")}</div>
                )}
              </div>
              {r?.updatedAt && <div className="module-foot">{tr(`Updated ${relTime(r.updatedAt, locale)}`, `${relTime(r.updatedAt, locale)}更新`)}</div>}
              {editing && (
                <>
                  <div className="resize-x" onPointerDown={(e) => startResize(e, t, "x")} />
                  <div className="resize-y" onPointerDown={(e) => startResize(e, t, "y")} />
                </>
              )}
            </div>
          );
        })}
      </div>

      {editing && (
        <div className="add-modules">
          <div className="muted small">{tr("Add modules", "添加模块")}</div>
          {offBoard.length === 0 ? (
            <div className="muted small">{tr("All tasks are already on the dashboard.", "所有任务都已在仪表盘上。")}</div>
          ) : (
            <div className="chips">
              {offBoard.map((t) => (
                <button key={t.id} className="btn small" onClick={() => addToBoard(t.id)}>
                  <Plus size={13} /> {t.nickname}
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      {displayed.length === 0 && !editing && (
        <div className="empty">
          {tr("Your dashboard is empty.", "仪表盘还是空的。")}
          {snap.tasks.length === 0 ? (
            <button className="btn" style={{ marginLeft: 8 }} onClick={() => onNavigate?.("tasks")}>
              {tr("Create task", "新建任务")}
            </button>
          ) : (
            <button className="btn" style={{ marginLeft: 8 }} onClick={startEdit}>
              {tr("Add module", "添加模块")}
            </button>
          )}
        </div>
      )}

      {openTask && (
        <TaskDetail
          task={openTask}
          result={snap.results[openTask.id]}
          executions={snap.executions.filter((e) => e.taskId === openTask.id)}
          onClose={() => setOpenTask(null)}
        />
      )}
    </div>
  );
}
