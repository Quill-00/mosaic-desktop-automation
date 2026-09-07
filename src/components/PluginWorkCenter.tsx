import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ChevronDown, Power, Puzzle } from "lucide-react";
import { api, errorMessage } from "../api";
import type { Snapshot, Task } from "../types";
import { useI18n } from "../i18n";
import CpaQuota from "./CpaQuota";
import PromptCleanerWorkbench from "./PromptCleanerWorkbench";
import ImageMetadataCleanerWorkbench from "./ImageMetadataCleanerWorkbench";

export default function PluginWorkCenter({ snap, preferredPluginId }: { snap: Snapshot; preferredPluginId?: string | null }) {
  const { t } = useI18n();
  const plugins = snap.tasks.filter((task) => task.active && task.kind === "plugin");
  const [openId, setOpenId] = useState<string | null>(preferredPluginId ?? null);
  const [message, setMessage] = useState("");
  const [dropActive, setDropActive] = useState(false);
  const dropPluginRef = useRef<Task | null>(null);
  const runningIds = new Set(snap.running.map((process) => process.taskId));

  useEffect(() => {
    if (preferredPluginId && plugins.some((plugin) => plugin.id === preferredPluginId)) setOpenId(preferredPluginId);
  }, [plugins, preferredPluginId]);

  useEffect(() => {
    const plugin = plugins.find((item) => item.id === openId) ?? null;
    dropPluginRef.current = plugin && snap.pluginSurfaces?.[plugin.id]?.drawer?.drop ? plugin : null;
  }, [openId, plugins, snap.pluginSurfaces]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWindow().onDragDropEvent((event) => {
      const plugin = dropPluginRef.current;
      if (!plugin) return;
      if (event.payload.type === "enter" || event.payload.type === "over") setDropActive(true);
      if (event.payload.type === "leave") setDropActive(false);
      if (event.payload.type === "drop") {
        setDropActive(false);
        setMessage(`已接收 ${event.payload.paths.length} 个路径，正在处理。`);
        void api.runPluginWithFiles(plugin.id, event.payload.paths)
          .then(() => setMessage("处理已开始，可在运行记录查看结果。"))
          .catch((error) => setMessage(errorMessage(error)));
      }
    }).then((fn) => { unlisten = fn; });
    return () => unlisten?.();
  }, []);

  return <div className="view plugin-center-view">
    <div className="view-head"><div><h2 className="view-title">{t("Work center", "工作中心")}</h2><p className="view-subtitle">{t("Select a plugin to expand its controls.", "点击插件即可展开操作。")}</p></div></div>
    <div className="work-plugin-list">
      {plugins.map((plugin) => {
        const open = openId === plugin.id;
        const running = runningIds.has(plugin.id);
        const surface = snap.pluginSurfaces?.[plugin.id];
        return <section className={"work-plugin-entry" + (open ? " open" : "")} key={plugin.id}>
          <div className="work-plugin-row">
            <button className="work-plugin-main" onClick={() => { setOpenId(open ? null : plugin.id); setMessage(""); }} aria-expanded={open}>
              <span className="widget-tile"><Puzzle /></span>
              <span className="work-plugin-label"><strong>{plugin.nickname}</strong><small><i className={"status-dot " + (running ? "online" : "unknown")} />{running ? t("Running", "运行中") : t("Ready", "就绪")}</small></span>
              <ChevronDown className={open ? "is-open" : ""} size={16} />
            </button>
            {plugin.lifecycle === "resident" && <button className={"widget-button" + (running ? " on" : "")} onClick={() => void api.setEnabled(plugin.id, !plugin.enabled)} aria-label={plugin.enabled ? t("Stop plugin", "停止插件") : t("Start plugin", "启动插件")}><Power /></button>}
          </div>
            {open && <div className="work-plugin-panel">
              {plugin.id === "plugin-prompt-cleaner" ? <PromptCleanerWorkbench />
              : plugin.id === "plugin-image-metadata-cleaner" ? <ImageMetadataCleanerWorkbench />
              : plugin.id === "plugin-cliproxyapi" ? <CpaQuota />
              : surface?.drawer ? <div className="work-plugin-generic">
                {surface.drawer.description && <p>{surface.drawer.description}</p>}
                {surface.drawer.drop && <div className={"plugin-center-drop" + (dropActive ? " active" : "")}><strong>{surface.drawer.drop.prompt || t("Drop files here", "把文件拖到这里")}</strong><span>{message || surface.drawer.drop.note}</span><small>{surface.drawer.drop.extensions.join(" · ")}</small></div>}
                {surface.drawer.actions.length > 0 && <div className="plugin-center-actions">{surface.drawer.actions.map((action) => <button className="btn" key={action.id} onClick={() => void api.runPluginAction(plugin.id, action.id)}>{action.label}</button>)}</div>}
              </div>
              : <p className="work-plugin-empty">{t("This plugin has no additional controls.", "该插件没有额外操作项。")}</p>}
          </div>}
        </section>;
      })}
      {plugins.length === 0 && <div className="empty">{t("No active plugins are installed.", "当前没有已启用的插件。")}</div>}
    </div>
  </div>;
}
