import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Image, Play, Trash2, X } from "lucide-react";
import { api, errorMessage } from "../api";

type SaveMode = "prefix" | "folder" | "overwrite";

const STORAGE_KEY = "mosaic.image-metadata-cleaner.settings";

function initialSettings(): { mode: SaveMode; value: string } {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) || "null") as { mode?: SaveMode; value?: string } | null;
    if (saved && ["prefix", "folder", "overwrite"].includes(saved.mode || "")) {
      return { mode: saved.mode!, value: saved.value || "" };
    }
  } catch {
    // A damaged preference should not prevent the plugin from opening.
  }
  return { mode: "prefix", value: "cleaned_" };
}

export default function ImageMetadataCleanerWorkbench({ compact = false }: { compact?: boolean }) {
  const initial = useRef(initialSettings());
  const [paths, setPaths] = useState<string[]>([]);
  const [mode, setMode] = useState<SaveMode>(initial.current.mode);
  const [value, setValue] = useState(initial.current.value);
  const [dropActive, setDropActive] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("拖入图片后再点击开始清洗，不会立即执行。");
  const [isError, setIsError] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") setDropActive(true);
      if (event.payload.type === "leave") setDropActive(false);
      if (event.payload.type === "drop") {
        const droppedPaths = event.payload.paths;
        setDropActive(false);
        setPaths((current) => Array.from(new Set([...current, ...droppedPaths])));
        setMessage(`已加入 ${droppedPaths.length} 个路径，等待处理。`);
        setIsError(false);
      }
    }).then((fn) => { unlisten = fn; });
    return () => unlisten?.();
  }, []);

  function changeMode(next: SaveMode) {
    setMode(next);
    if (next === "prefix" && (!value || value === "清洗后")) setValue("cleaned_");
    if (next === "folder" && (!value || value === "cleaned_")) setValue("清洗后");
  }

  async function run() {
    if (paths.length === 0) {
      setMessage("请先拖入至少一张图片。");
      setIsError(true);
      return;
    }
    setBusy(true);
    setIsError(false);
    setMessage("正在提交清洗任务...");
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ mode, value }));
      await api.runImageMetadataCleaner(paths, mode, value);
      setMessage(`已开始处理 ${paths.length} 个路径，可在运行记录查看输出。`);
      setPaths([]);
    } catch (error) {
      setMessage(errorMessage(error));
      setIsError(true);
    } finally {
      setBusy(false);
    }
  }

  return <section className={"image-cleaner-workbench" + (compact ? " compact" : "")}>
    <div className={"image-cleaner-drop" + (dropActive ? " active" : "")}>
      <Image size={21} />
      <strong>把多张图片拖到这里</strong>
      <span>支持 JPG、PNG、WebP，也可拖入包含图片的文件夹</span>
    </div>

    <div className="image-cleaner-settings">
      <label><span>副本保存方式</span><select value={mode} onChange={(event) => changeMode(event.target.value as SaveMode)}>
        <option value="prefix">原目录，添加文件名前缀</option>
        <option value="folder">原目录下的子文件夹，文件名不变</option>
        <option value="overwrite">直接覆盖原图</option>
      </select></label>
      {mode !== "overwrite" && <label><span>{mode === "prefix" ? "文件名前缀" : "子目录名称"}</span><input value={value} maxLength={80} onChange={(event) => setValue(event.target.value)} /></label>}
    </div>

    <div className="image-cleaner-queue-head">
      <strong>待处理队列</strong>
      <span>{paths.length} 个路径</span>
      {paths.length > 0 && <button onClick={() => setPaths([])}><Trash2 size={14} /> 清空</button>}
    </div>
    <div className="image-cleaner-queue">
      {paths.map((path) => <div key={path}><span title={path}>{path}</span><button onClick={() => setPaths((current) => current.filter((item) => item !== path))} aria-label={`移除 ${path}`}><X size={14} /></button></div>)}
      {paths.length === 0 && <p>还没有图片，拖入后会在这里列出。</p>}
    </div>

    <footer className="image-cleaner-foot">
      <p className={isError ? "error" : ""}>{message}</p>
      <button className="btn primary" disabled={busy || paths.length === 0} onClick={() => void run()}><Play size={15} /> {busy ? "正在提交..." : "开始清洗"}</button>
    </footer>
  </section>;
}
