import { useEffect, useState } from "react";
import { AlertTriangle, Bell, Check, Trash2, X } from "lucide-react";
import { api } from "../api";
import { relTime } from "../api";
import type { Snapshot } from "../types";
import { useI18n } from "../i18n";

export default function NotificationsView({ snap }: { snap: Snapshot }) {
  const { locale, t } = useI18n();
  const [confirmClear, setConfirmClear] = useState(false);
  const items = snap.notifications;
  const unread = items.filter((item) => !item.read).length;
  const read = items.length - unread;

  useEffect(() => {
    if (!confirmClear) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setConfirmClear(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [confirmClear]);

  return (
    <div className="view">
      <div className="view-head notification-head">
        <h2 className="view-title">{t("Notifications", "通知")}</h2>
        <span className="muted">{t(`${unread} unread`, `${unread} 未读`)}</span>
        {unread > 0 && (
          <button className="btn" onClick={() => api.markAllRead()}>
            <Check size={14} /> {t("Mark all read", "全部已读")}
          </button>
        )}
        {read > 0 && (
          <button className="btn" onClick={() => api.clearNotifications(true)}>
            <Trash2 size={14} /> {t("Clear read", "清理已读")}
          </button>
        )}
        {items.length > 0 && (
          <button className="btn danger" onClick={() => setConfirmClear(true)}>
            <Trash2 size={14} /> {t("Clear all", "清空全部")}
          </button>
        )}
      </div>

      {items.length === 0 && <div className="empty">{t("No notifications.", "通知已经清理干净。")}</div>}

      <div className="rows">
        {items.map((item) => (
          <div
            key={item.id}
            className={"row notif" + (item.read ? " read" : "")}
            onClick={() => !item.read && api.markRead(item.id)}
          >
            <div className="row-main">
              {item.level === "danger" ? (
                <AlertTriangle size={16} className="danger-ic" />
              ) : (
                <Bell size={16} className="muted" />
              )}
              <span className="row-name">{item.title}</span>
              {!item.read && <span className="dot info" style={{ marginLeft: "auto" }} />}
              <button
                className="icon-btn notification-delete"
                onClick={(event) => {
                  event.stopPropagation();
                  void api.deleteNotification(item.id);
                }}
                aria-label={t(`Delete notification: ${item.title}`, `删除通知：${item.title}`)}
                title={t("Delete notification", "删除通知")}
              >
                <X size={14} />
              </button>
            </div>
            {item.body && (
              <div className="row-meta">
                <span>{item.body}</span>
              </div>
            )}
            <div className="row-meta">
              <span>{relTime(item.at, locale)}</span>
            </div>
          </div>
        ))}
      </div>

      {confirmClear && (
        <div className="modal" onClick={() => setConfirmClear(false)}>
          <div className="sheet confirm-sheet" role="alertdialog" aria-modal="true" aria-labelledby="clear-notifications-title" onClick={(event) => event.stopPropagation()}>
            <h3 id="clear-notifications-title">{t("Clear all notifications?", "清空全部通知？")}</h3>
            <p className="muted small">{t("This only removes notification records. Script outputs, run history, and plugin settings are preserved.", "只删除通知记录，不会删除脚本产物、运行记录或插件配置。")}</p>
            <div className="sheet-actions">
              <button className="btn" onClick={() => setConfirmClear(false)}>{t("Cancel", "取消")}</button>
              <button
                className="btn danger"
                onClick={() => {
                  void api.clearNotifications(false);
                  setConfirmClear(false);
                }}
              >
                <Trash2 size={14} /> {t("Clear all", "清空全部")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
