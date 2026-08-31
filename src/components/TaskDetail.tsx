import { useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { FileText, Folder, X } from "lucide-react";
import { api, errorMessage, relTime } from "../api";
import type { DetailItem, Execution, Task, TaskResultState } from "../types";
import { useI18n } from "../i18n";

const STATUS: Record<string, { en: string; zh: string; cls: string }> = {
  ok: { en: "Succeeded", zh: "成功", cls: "ok" },
  running: { en: "Running", zh: "运行中", cls: "info" },
  failed: { en: "Failed", zh: "失败", cls: "danger" },
  timedOut: { en: "Timed out", zh: "超时", cls: "danger" },
  killed: { en: "Terminated", zh: "已终止", cls: "danger" },
};

export default function TaskDetail({
  task,
  executions,
  onClose,
}: {
  task: Task;
  result?: TaskResultState;
  executions: Execution[];
  onClose: () => void;
}) {
  const { locale, t } = useI18n();
  const rounds = executions; // already filtered to this task, newest first
  const [selId, setSelId] = useState<string | null>(rounds[0]?.id ?? null);
  const [cache, setCache] = useState<Record<string, DetailItem[]>>({});
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState("");
  const [preview, setPreview] = useState<DetailItem | null>(null);

  // Keep a valid selection as the snapshot (and thus `rounds`) refreshes.
  useEffect(() => {
    if (rounds.length && !rounds.some((r) => r.id === selId)) setSelId(rounds[0].id);
  }, [rounds, selId]);

  const sel = rounds.find((r) => r.id === selId) || null;
  const selStatus = sel?.status;

  // Fetch the selected round's products on demand. Refetch when the round
  // finishes (running -> ok) so freshly produced items show up.
  useEffect(() => {
    if (!selId) return;
    let live = true;
    setLoading(true);
    setLoadError("");
    api
      .execItems(selId)
      .then((items) => {
        if (live) setCache((c) => ({ ...c, [selId]: items }));
      })
      .catch((error) => {
        if (live) setLoadError(errorMessage(error));
      })
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [selId, selStatus]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (preview) setPreview(null);
      else onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, preview]);

  const items = (selId && cache[selId]) || [];

  const groups = useMemo(() => {
    const has = (k: string) => (i: DetailItem) => i.kind === k && !!i.path;
    return {
      imgs: items.filter(has("image")),
      vids: items.filter(has("video")),
      auds: items.filter(has("audio")),
      files: items.filter(has("file")),
      texts: items.filter((i) => !i.path),
    };
  }, [items]);

  const hasDir = !!(task.outputDir && task.outputDir.trim());

  return (
    <div className="modal" onClick={onClose}>
      <div className="sheet wide detail" role="dialog" aria-modal="true" aria-labelledby="task-detail-title" onClick={(e) => e.stopPropagation()}>
        <div className="sheet-head">
          <h3 id="task-detail-title" style={{ flex: 1 }}>{task.nickname} · {t("Outputs", "产物")}</h3>
          {hasDir && (
            <button className="btn small" onClick={() => api.openPath(task.outputDir as string)}>
              <Folder size={14} /> {t("Open output folder", "打开产物目录")}
            </button>
          )}
          <button className="icon-btn" onClick={onClose} aria-label={t("Close output details", "关闭产物详情")}>
            <X size={16} />
          </button>
        </div>

        {rounds.length === 0 ? (
          <div className="empty">{t("No run history yet. Outputs will appear here after the task runs.", "还没有运行记录。运行一次任务后，这里会按轮次显示产物。")}</div>
        ) : (
          <div className="detail-layout">
            <div className="rounds">
              {rounds.map((r) => {
                const s = STATUS[r.status] || { en: r.status, zh: r.status, cls: "info" };
                return (
                  <button
                    key={r.id}
                    className={"round" + (r.id === selId ? " active" : "")}
                    onClick={() => setSelId(r.id)}
                  >
                    <div className="round-top">
                      <span className={"dot " + s.cls} />
                      <span className="round-time">{relTime(r.startedAt, locale) || r.startedAt}</span>
                    </div>
                    <div className="round-sub">
                      {locale === "zh-CN" ? s.zh : s.en}
                      {r.itemCount > 0 ? t(` · ${r.itemCount} items`, ` · ${r.itemCount} 项`) : ""}
                      {r.trigger ? ` · ${r.trigger}` : ""}
                    </div>
                  </button>
                );
              })}
            </div>

            <div className="round-content">
              {loading && items.length === 0 && <div className="muted small">{t("Loading…", "加载中…")}</div>}
              {loadError && <div className="form-error" role="alert">{loadError}</div>}

              {!loading && sel && items.length === 0 && (
                <div className="empty">
                  {sel.status === "running"
                    ? t("This run is still in progress…", "本轮正在运行…")
                    : sel.status !== "ok"
                      ? sel.error || t("This run did not succeed, so there are no outputs.", "本轮未成功，没有产物。")
                      : t("This run produced no displayable outputs.", "本轮没有可展示的产物。")}
                  {hasDir && sel.status === "ok" && (
                    <button
                      className="btn small"
                      style={{ marginLeft: 8 }}
                      onClick={() => api.openPath(task.outputDir as string)}
                    >
                      <Folder size={14} /> {t("Open folder", "打开目录")}
                    </button>
                  )}
                </div>
              )}

              {groups.texts.length > 0 && (
                <div className="rows-list">
                  {groups.texts.map((it, i) => (
                    <div key={i} className="tl-row">
                      <div className="tl-title">{it.title}</div>
                      {it.subtitle && <div className="li-sub">{it.subtitle}</div>}
                      {it.at && <div className="tl-time">{relTime(it.at, locale)}</div>}
                    </div>
                  ))}
                </div>
              )}

              {groups.imgs.length > 0 && (
                <div className="gallery">
                  {groups.imgs.map((it, i) => (
                    <button key={i} className="thumb" onClick={() => setPreview(it)}>
                      <img src={convertFileSrc(it.path as string)} loading="lazy" alt={it.title} />
                      <div className="thumb-cap">{it.title}</div>
                    </button>
                  ))}
                </div>
              )}

              {groups.vids.length > 0 && (
                <div className="media-grid">
                  {groups.vids.map((it, i) => (
                    <div key={i} className="media-cell">
                      <video src={convertFileSrc(it.path as string)} controls preload="metadata" />
                      <div className="thumb-cap">{it.title}</div>
                    </div>
                  ))}
                </div>
              )}

              {groups.auds.length > 0 && (
                <div className="rows-list">
                  {groups.auds.map((it, i) => (
                    <div key={i} className="audio-row">
                      <div className="tl-title">{it.title}</div>
                      <audio src={convertFileSrc(it.path as string)} controls preload="none" />
                    </div>
                  ))}
                </div>
              )}

              {groups.files.length > 0 && (
                <div className="rows-list">
                  {groups.files.map((it, i) => (
                    <button
                      key={i}
                      className="file-row"
                      onClick={() => it.path && api.openPath(it.path)}
                      title={t("Open file", "打开文件")}
                    >
                      <FileText size={15} className="muted" />
                      <span className="tl-title">{it.title}</span>
                      {it.subtitle && <span className="li-sub">{it.subtitle}</span>}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {preview && preview.path && (
        <div
          className="preview-overlay"
          role="button"
          aria-label={t("Close image preview", "关闭图片预览")}
          tabIndex={0}
          onClick={(e) => {
            e.stopPropagation();
            setPreview(null);
          }}
        >
          <img src={convertFileSrc(preview.path)} alt={preview.title} />
          <div className="preview-cap">{preview.title}</div>
        </div>
      )}
    </div>
  );
}
