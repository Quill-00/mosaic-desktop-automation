import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import type {
  BotChannel,
  Brief,
  CapabilityProfile,
  ChannelInfo,
  CommunityCatalog,
  DetailItem,
  EntryGuess,
  PopoConfig,
  PopoPeer,
  Snapshot,
  Task,
  WindowConfig,
} from "./types";

export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  execItems: (execId: string) => invoke<DetailItem[]>("exec_items", { execId }),
  openPath: (path: string) => invoke("open_path", { path }),
  createTask: (task: Task) => invoke<Task>("create_task", { task }),
  deleteTask: (id: string, deleteProducts = false) => invoke("delete_task", { id, deleteProducts }),
  setActive: (id: string, active: boolean) => invoke("set_active", { id, active }),
  setEnabled: (id: string, enabled: boolean) => invoke("set_enabled", { id, enabled }),
  runNow: (id: string) => invoke("run_now", { id }),
  setDashboard: (orderedIds: string[]) => invoke("set_dashboard", { orderedIds }),
  setModuleSpan: (id: string, col: number, row: number) => invoke("set_module_span", { id, col, row }),
  terminate: (id: string) => invoke("terminate", { id }),
  terminateAll: () => invoke("terminate_all"),
  markRead: (id: string) => invoke("mark_read", { id }),
  markAllRead: () => invoke("mark_all_read"),
  deleteNotification: (id: string) => invoke("delete_notification", { id }),
  clearNotifications: (readOnly: boolean) => invoke("clear_notifications", { readOnly }),
  scanScript: (source: string) => invoke<CapabilityProfile>("scan_script", { source }),
  scanTaskSource: (command: string, args: string[], cwd?: string | null) =>
    invoke<CapabilityProfile>("scan_task_source", { command, args, cwd }),
  inspectTarget: (path: string) => invoke<EntryGuess>("inspect_target", { path }),
  importLocal: (path: string) => invoke<EntryGuess>("import_local", { path }),
  listChannels: () => invoke<ChannelInfo[]>("list_channels"),
  dailyBrief: () => invoke<Brief>("daily_brief"),
  savePopoConfig: (config: PopoConfig) => invoke("save_popo_config", { config }),
  popoScan: () => invoke<PopoPeer[]>("popo_scan"),
  sendToPopo: (text: string) => invoke("send_to_popo", { text }),
  saveBotChannel: (channel: BotChannel, secret: string) =>
    invoke<BotChannel>("save_bot_channel", { channel, secret }),
  setBotChannelEnabled: (id: string, enabled: boolean) =>
    invoke("set_bot_channel_enabled", { id, enabled }),
  testBotChannel: (id: string) => invoke<string>("test_bot_channel", { id }),
  deleteBotChannel: (id: string) => invoke("delete_bot_channel", { id }),
  saveWindowConfig: (config: WindowConfig) => invoke("save_window_config", { config }),
  saveCommunitySources: (sources: string[]) => invoke("save_community_sources", { sources }),
  communityCatalog: () => invoke<CommunityCatalog>("community_catalog"),
  installCommunityPackage: (sourceUrl: string, packageId: string, version: string) =>
    invoke<Task>("install_community_package", { sourceUrl, packageId, version }),
  uninstallCommunityPackage: (packageId: string) =>
    invoke("uninstall_community_package", { packageId }),
  checkForUpdates: () => invoke("check_for_updates"),
};

export function errorMessage(error: unknown, fallback = "Operation failed. Please try again."): string {
  if (error instanceof Error && error.message.trim()) return error.message.trim();
  if (typeof error === "string" && error.trim()) return error.trim();
  try {
    const text = JSON.stringify(error);
    if (text && text !== "{}") return text;
  } catch {
    // Fall through to a useful, user-facing default.
  }
  return fallback;
}

export function useSnapshot() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const inFlight = useRef(false);
  const refresh = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    try {
      setSnap(await api.snapshot());
      setError(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      inFlight.current = false;
    }
  }, []);
  useEffect(() => {
    void refresh();
    let active = true;
    let unlisten: (() => void) | undefined;
    listen("mosaic:changed", () => void refresh())
      .then((fn) => {
        if (active) unlisten = fn;
        else fn();
      })
      .catch(() => {
        // Polling remains the fallback when the native event bridge is unavailable.
      });
    const timer = setInterval(refresh, 2000);
    return () => {
      active = false;
      unlisten?.();
      clearInterval(timer);
    };
  }, [refresh]);
  return { snap, error, refresh };
}

export function relTime(iso?: string | null, locale: "en" | "zh-CN" = "en"): string {
  if (!iso) return "";
  const t = new Date(iso).getTime();
  if (isNaN(t)) return "";
  const s = Math.floor((Date.now() - t) / 1000);
  if (locale === "zh-CN") {
    if (s < 60) return "刚刚";
    if (s < 3600) return `${Math.floor(s / 60)} 分钟前`;
    if (s < 86400) return `${Math.floor(s / 3600)} 小时前`;
    return `${Math.floor(s / 86400)} 天前`;
  }
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} hr ago`;
  return `${Math.floor(s / 86400)} d ago`;
}

export function fmtDuration(secs: number): string {
  const v = Math.max(0, Math.floor(secs));
  if (v < 60) return `${v}s`;
  const m = Math.floor(v / 60);
  const s = v % 60;
  if (m < 60) return `${m}:${String(s).padStart(2, "0")}`;
  const h = Math.floor(m / 60);
  const mm = m % 60;
  return `${h}:${String(mm).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}
