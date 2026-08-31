import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { currentMonitor, getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import { Activity, AlertCircle, Bell, Blocks, LayoutGrid, ListChecks, Plus, RefreshCw, Settings, ShieldAlert, X } from "lucide-react";
import { errorMessage, useSnapshot } from "./api";
import { useI18n } from "./i18n";
import Dashboard from "./components/Dashboard";
import Tasks from "./components/Tasks";
import Running from "./components/Running";
import NotificationsView from "./components/Notifications";
import SettingsView from "./components/Settings";
import CommunityCenter from "./components/CommunityCenter";
import WidgetView from "./components/WidgetView";
import "./styles.css";

type View = "dashboard" | "tasks" | "community" | "running" | "notifications" | "settings";

const IS_WIDGET = (() => {
  try {
    return getCurrentWindow().label === "widget";
  } catch {
    return false;
  }
})();

export default function App() {
  const { t } = useI18n();
  const { snap, error, refresh } = useSnapshot();
  const [view, setView] = useState<View>("dashboard");
  const [autoNew, setAutoNew] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const edgeHide = snap?.window?.edgeHide ?? false;

  // QQ-style edge auto-hide for the main window: drag it to a screen edge and it
  // slides off, leaving a sliver; hovering the sliver slides it back.
  useEffect(() => {
    if (IS_WIDGET || !edgeHide) return;
    const w = getCurrentWindow();
    const PEEK = 5;
    let dock: "left" | "right" | "top" | null = null;
    let hidden = false;
    let unMoved: (() => void) | undefined;
    let leaveTimer: ReturnType<typeof setTimeout> | undefined;

    async function recompute() {
      if (hidden) return;
      const mon = await currentMonitor();
      if (!mon) return;
      const pos = await w.outerPosition();
      const size = await w.outerSize();
      const { x: mx, y: my } = mon.position;
      const { width: mw } = mon.size;
      if (pos.y <= my + 2) dock = "top";
      else if (pos.x <= mx + 2) dock = "left";
      else if (pos.x + size.width >= mx + mw - 2) dock = "right";
      else dock = null;
    }
    async function hide() {
      if (hidden || !dock) return;
      const pos = await w.outerPosition();
      const size = await w.outerSize();
      if (dock === "top") await w.setPosition(new PhysicalPosition(pos.x, pos.y - size.height + PEEK));
      else if (dock === "left") await w.setPosition(new PhysicalPosition(pos.x - size.width + PEEK, pos.y));
      else if (dock === "right") await w.setPosition(new PhysicalPosition(pos.x + size.width - PEEK, pos.y));
      hidden = true;
    }
    async function reveal() {
      if (!hidden) return;
      const mon = await currentMonitor();
      const pos = await w.outerPosition();
      const size = await w.outerSize();
      if (mon) {
        if (dock === "top") await w.setPosition(new PhysicalPosition(pos.x, mon.position.y));
        else if (dock === "left") await w.setPosition(new PhysicalPosition(mon.position.x, pos.y));
        else if (dock === "right")
          await w.setPosition(new PhysicalPosition(mon.position.x + mon.size.width - size.width, pos.y));
      }
      hidden = false;
    }

    const onEnter = () => {
      if (leaveTimer) clearTimeout(leaveTimer);
      reveal();
    };
    const onLeave = () => {
      if (!dock) return;
      if (leaveTimer) clearTimeout(leaveTimer);
      leaveTimer = setTimeout(hide, 500);
    };

    w.onMoved(() => recompute()).then((u) => (unMoved = u));
    document.addEventListener("mouseenter", onEnter);
    document.addEventListener("mouseleave", onLeave);

    return () => {
      unMoved?.();
      document.removeEventListener("mouseenter", onEnter);
      document.removeEventListener("mouseleave", onLeave);
      if (leaveTimer) clearTimeout(leaveTimer);
      reveal();
    };
  }, [edgeHide]);

  useEffect(() => {
    const onRejection = (event: PromiseRejectionEvent) => {
      event.preventDefault();
      setActionError(errorMessage(event.reason, t("Operation failed. Please try again.", "操作失败，请稍后重试。")));
    };
    window.addEventListener("unhandledrejection", onRejection);
    return () => window.removeEventListener("unhandledrejection", onRejection);
  }, [t]);

  useEffect(() => {
    if (!actionError) return;
    const timer = window.setTimeout(() => setActionError(null), 7000);
    return () => window.clearTimeout(timer);
  }, [actionError]);

  if (IS_WIDGET) {
    return <WidgetView />;
  }

  const running = snap?.running.length ?? 0;
  const unread = snap?.notifications.filter((n) => !n.read).length ?? 0;

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-dot" />
          <span className="brand-word">Mosaic</span>
        </div>
        <button
          className="new-task-btn"
          onClick={() => {
            setView("tasks");
            setAutoNew(true);
          }}
        >
          <Plus size={17} /> {t("Add script or plugin", "添加脚本或插件")}
        </button>
        <div className="sidebar-divider" />
        <nav>
          <NavBtn icon={<LayoutGrid size={17} />} label={t("Dashboard", "仪表盘")} active={view === "dashboard"} onClick={() => setView("dashboard")} />
          <NavBtn icon={<ListChecks size={17} />} label={t("Scripts & plugins", "脚本与插件")} active={view === "tasks"} onClick={() => setView("tasks")} />
          <NavBtn icon={<Blocks size={17} />} label={t("Community", "插件中心")} active={view === "community"} onClick={() => setView("community")} />
          <NavBtn icon={<Activity size={17} />} label={t("Running", "正在运行")} badge={running || undefined} active={view === "running"} onClick={() => setView("running")} />
          <NavBtn icon={<Bell size={17} />} label={t("Notifications", "通知")} badge={unread || undefined} active={view === "notifications"} onClick={() => setView("notifications")} />
          <NavBtn icon={<Settings size={17} />} label={t("Settings", "设置")} active={view === "settings"} onClick={() => setView("settings")} />
        </nav>
        <div className="sidebar-foot">
          <ShieldAlert size={14} />
          {t("Local execution · Trust third-party code", "本地执行 · 第三方代码需信任")}
        </div>
      </aside>

      <main className="content">
        {!snap && !error && (
          <div className="loading-state" role="status">
            <RefreshCw size={18} className="spin" />
            <span>{t("Connecting to the Mosaic engine…", "正在连接 Mosaic 引擎…")}</span>
          </div>
        )}
        {!snap && error && (
          <div className="engine-error" role="alert">
            <AlertCircle size={22} />
            <div>
              <strong>{t("Cannot connect to the Mosaic engine", "无法连接 Mosaic 引擎")}</strong>
              <p>{error}</p>
            </div>
            <button className="btn" onClick={() => void refresh()}>
              <RefreshCw size={14} /> {t("Retry", "重试")}
            </button>
          </div>
        )}
        {snap && error && (
          <div className="connection-banner" role="status">
            <AlertCircle size={14} />
            <span>{t("Refresh is temporarily unavailable. Showing the latest cached result.", "数据刷新暂时中断，当前显示的是最近一次结果。")}</span>
            <button className="btn small" onClick={() => void refresh()}>{t("Retry", "重试")}</button>
          </div>
        )}
        {snap && view === "dashboard" && <Dashboard snap={snap} onNavigate={(v) => setView(v as View)} />}
        {snap && view === "tasks" && <Tasks snap={snap} autoNew={autoNew} onAutoNew={() => setAutoNew(false)} />}
        {snap && view === "community" && <CommunityCenter snap={snap} />}
        {snap && view === "running" && <Running snap={snap} />}
        {snap && view === "notifications" && <NotificationsView snap={snap} />}
        {snap && view === "settings" && <SettingsView snap={snap} />}
      </main>

      {actionError && (
        <div className="toast danger-toast" role="alert">
          <AlertCircle size={16} />
          <span>{actionError}</span>
          <button className="icon-btn" onClick={() => setActionError(null)} aria-label={t("Dismiss error", "关闭错误提示")}>
            <X size={15} />
          </button>
        </div>
      )}
    </div>
  );
}

function NavBtn(props: {
  icon: ReactNode;
  label: string;
  active: boolean;
  onClick: () => void;
  badge?: number;
}) {
  return (
    <button
      className={"nav" + (props.active ? " active" : "")}
      onClick={props.onClick}
      aria-current={props.active ? "page" : undefined}
      title={props.label}
    >
      {props.icon}
      <span>{props.label}</span>
      {props.badge ? <span className="badge">{props.badge}</span> : null}
    </button>
  );
}
