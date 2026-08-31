import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
import { Boxes, ChevronRight, ExternalLink, Play, Power, Puzzle } from "lucide-react";
import { api, useSnapshot } from "../api";
import type { Task } from "../types";

function scriptStatus(task: Task, running: boolean, headline?: string): string {
  if (running) return "正在运行";
  if (headline) return headline;
  return task.enabled ? "等待运行" : "已停用";
}

export default function WidgetView() {
  const { snap, error, refresh } = useSnapshot();
  const [expanded, setExpanded] = useState(false);
  const [switching, setSwitching] = useState<string | null>(null);
  const collapseTimer = useRef<number | null>(null);
  const collapseGuard = useRef(0);
  const dragging = useRef(false);
  const drag = useRef<{
    pointerId: number;
    screenX: number;
    screenY: number;
    windowX: number;
    windowY: number;
  } | null>(null);
  const dragFrame = useRef<number | null>(null);
  const pendingPosition = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    document.documentElement.dataset.surface = "widget";
    return () => {
      if (dragFrame.current !== null) window.cancelAnimationFrame(dragFrame.current);
      delete document.documentElement.dataset.surface;
    };
  }, []);

  const runningIds = new Set(snap?.running.map((process) => process.taskId) ?? []);
  const plugins = snap?.tasks.filter((task) => task.active && task.kind === "plugin") ?? [];
  const scripts = snap
    ? snap.tasks
        .filter((task) => task.active && task.kind !== "plugin" && task.onDashboard !== false)
        .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
    : [];

  function expand() {
    if (collapseTimer.current) window.clearTimeout(collapseTimer.current);
    collapseTimer.current = null;
    if (expanded) return;
    // Resizing a 12 px edge strip around the pointer can briefly generate a
    // synthetic mouseleave. Guard the first frames just like The Tower does.
    collapseGuard.current = Date.now() + 700;
    setExpanded(true);
    void invoke("set_widget_expanded", { expanded: true });
    void refresh();
  }

  function collapse() {
    if (collapseTimer.current) window.clearTimeout(collapseTimer.current);
    collapseTimer.current = null;
    setExpanded(false);
    void invoke("set_widget_expanded", { expanded: false });
  }

  function scheduleCollapse() {
    if (dragging.current) return;
    if (collapseTimer.current) window.clearTimeout(collapseTimer.current);
    const remaining = collapseGuard.current - Date.now();
    collapseTimer.current = window.setTimeout(collapse, Math.max(500, remaining));
  }

  useEffect(() => {
    if (!expanded) return;
    // Resizing/repositioning a transparent WebView can emit a transient blur.
    // Ignore it during the expansion guard; a real later focus loss still folds.
    const onBlur = () => {
      if (Date.now() >= collapseGuard.current) scheduleCollapse();
    };
    window.addEventListener("blur", onBlur);
    return () => window.removeEventListener("blur", onBlur);
  }, [expanded]);

  async function startDragging(event: React.PointerEvent<HTMLElement>) {
    if (event.button !== 0 || (event.target as HTMLElement).closest("button")) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    if (collapseTimer.current) window.clearTimeout(collapseTimer.current);

    const widget = getCurrentWindow();
    try {
      const [position, scaleFactor] = await Promise.all([
        widget.outerPosition(),
        widget.scaleFactor(),
      ]);
      drag.current = {
        pointerId: event.pointerId,
        screenX: event.screenX,
        screenY: event.screenY,
        windowX: position.x / scaleFactor,
        windowY: position.y / scaleFactor,
      };
      dragging.current = true;
    } catch {
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
      dragging.current = false;
    }
  }

  function moveDragging(event: React.PointerEvent<HTMLElement>) {
    const origin = drag.current;
    if (!origin || origin.pointerId !== event.pointerId) return;
    pendingPosition.current = {
      x: origin.windowX + event.screenX - origin.screenX,
      y: origin.windowY + event.screenY - origin.screenY,
    };
    if (dragFrame.current !== null) return;
    dragFrame.current = window.requestAnimationFrame(() => {
      dragFrame.current = null;
      const next = pendingPosition.current;
      if (next) void getCurrentWindow().setPosition(new LogicalPosition(next.x, next.y));
    });
  }

  async function finishDragging(event: React.PointerEvent<HTMLElement>) {
    const origin = drag.current;
    if (!origin || origin.pointerId !== event.pointerId) return;
    drag.current = null;
    if (dragFrame.current !== null) {
      window.cancelAnimationFrame(dragFrame.current);
      dragFrame.current = null;
    }
    const finalPosition = {
      x: origin.windowX + event.screenX - origin.screenX,
      y: origin.windowY + event.screenY - origin.screenY,
    };
    pendingPosition.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    try {
      // Apply the final pointer position first. Only after Windows confirms the
      // move do we decide which monitor edge is nearest. A deliberate horizontal
      // throw wins over the midpoint, so a compact gesture can cross the screen.
      await getCurrentWindow().setPosition(new LogicalPosition(finalPosition.x, finalPosition.y));
      const horizontalDelta = event.screenX - origin.screenX;
      const preferredEdge = Math.abs(horizontalDelta) >= 72
        ? horizontalDelta > 0 ? "right" : "left"
        : null;
      await invoke("snap_widget_to_edge", { preferredEdge });
    } finally {
      dragging.current = false;
      collapseGuard.current = Date.now() + 700;
    }
  }

  async function togglePlugin(task: Task) {
    if (switching) return;
    setSwitching(task.id);
    try {
      await api.setEnabled(task.id, !task.enabled);
    } finally {
      setSwitching(null);
    }
  }

  async function runScript(task: Task) {
    if (switching || runningIds.has(task.id)) return;
    setSwitching(task.id);
    try {
      await api.runNow(task.id);
    } finally {
      setSwitching(null);
    }
  }

  function openMain() {
    void invoke("show_main_window");
  }

  if (!expanded) {
    return (
      <button
        className="widget-handle"
        onMouseEnter={expand}
        onFocus={expand}
        onClick={expand}
        aria-label="展开 Mosaic 快捷挂件"
      >
        <span className="widget-handle-line" />
      </button>
    );
  }

  return (
    <section
      className="widget-panel"
      onMouseEnter={expand}
      onMouseLeave={scheduleCollapse}
      aria-label="Mosaic 快捷挂件"
    >
      <header
        className="widget-header"
        onPointerDown={(event) => void startDragging(event)}
        onPointerMove={moveDragging}
        onPointerUp={(event) => void finishDragging(event)}
        onPointerCancel={(event) => void finishDragging(event)}
        title="拖动后自动吸附到最近的屏幕边缘"
      >
        <div className="widget-title">
          <strong>脚本与插件</strong>
          <span>
            <i className={error ? "status-dot offline" : "status-dot online"} />
            {error ? "引擎连接中断" : `${runningIds.size} 项正在运行`}
          </span>
        </div>
        <div className="widget-actions">
          <button onClick={collapse} aria-label="收起挂件"><ChevronRight /></button>
        </div>
      </header>

      <div className="widget-scroll">
        {!snap && !error && <div className="widget-blank">正在连接 Mosaic…</div>}
        {error && (
          <div className="widget-blank">
            <span>暂时无法刷新运行状态</span>
            <button onClick={() => void refresh()}>重试</button>
          </div>
        )}

        {plugins.length > 0 && <div className="widget-section"><p>插件</p></div>}
        {plugins.map((plugin) => {
          const running = runningIds.has(plugin.id);
          return (
            <div className={"widget-row" + (running ? " on" : "")} key={plugin.id}>
              <div className="widget-tile"><Puzzle /></div>
              <div className="widget-label">
                <strong title={plugin.nickname}>{plugin.nickname}</strong>
                <span>
                  <i className={running ? "status-dot online" : "status-dot unknown"} />
                  {running ? "运行中" : plugin.enabled ? "正在启动" : "已关闭"}
                </span>
              </div>
              <button
                className={running ? "widget-button on" : "widget-button"}
                onClick={() => void togglePlugin(plugin)}
                disabled={switching === plugin.id}
                aria-label={`${plugin.enabled ? "关闭并终止" : "静默启动"}${plugin.nickname}`}
                title={plugin.enabled ? "关闭并终止整个进程树" : "静默启动"}
              >
                <Power />
              </button>
            </div>
          );
        })}

        {scripts.length > 0 && <div className="widget-section"><p>仪表盘脚本</p></div>}
        {scripts.map((script) => {
          const running = runningIds.has(script.id);
          const headline = snap?.results[script.id]?.summary?.headline;
          return (
            <div className={"widget-row" + (running ? " on" : !script.enabled ? " offline" : "")} key={script.id}>
              <div className="widget-tile"><Play /></div>
              <div className="widget-label">
                <strong title={script.nickname}>{script.nickname}</strong>
                <span>
                  <i className={running ? "status-dot online" : "status-dot unknown"} />
                  {scriptStatus(script, running, headline)}
                </span>
              </div>
              <button
                className={running ? "widget-button on" : "widget-button"}
                onClick={() => void runScript(script)}
                disabled={running || switching === script.id}
                aria-label={`运行${script.nickname}`}
              >
                <Play />
              </button>
            </div>
          );
        })}

        {snap && plugins.length === 0 && scripts.length === 0 && (
          <div className="widget-blank">
            <Boxes />
            <span>库里还没有可快捷控制的内容</span>
            <button onClick={openMain}>打开脚本与插件库</button>
          </div>
        )}
      </div>

      <footer className="widget-footer">
        <span>{plugins.length} 个插件 · {scripts.length} 个仪表盘脚本</span>
        <button onClick={openMain}>打开主库 <ExternalLink /></button>
      </footer>
    </section>
  );
}
