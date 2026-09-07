import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, RefreshCw, Settings2 } from "lucide-react";
import { useI18n } from "../i18n";

interface Connection { baseUrl: string; configured: boolean }
interface QuotaWindow {
  label: string; remainingPercent: number | null; resetAt: string | null;
  kind: "quota" | "billing"; windowSeconds: number | null;
}
interface AccountQuota {
  id: string; name: string; provider: string; plan: string;
  windows: QuotaWindow[]; error: string | null;
}
interface QuotaSnapshot {
  configured: boolean; accounts: AccountQuota[]; updatedAt: string | null;
  error: string | null; refreshing: boolean;
}

function explain(error: unknown, zh: boolean): string {
  const code = typeof error === "string" ? error : "connection_failed";
  const messages: Record<string, [string, string]> = {
    invalid_url: ["Enter a CPA base URL without credentials, query or fragment.", "请输入 CPA 基础地址，不含凭据、查询参数或片段。"],
    https_required: ["Use HTTPS for remote CPA; HTTP is allowed only on localhost.", "远程 CPA 请使用 HTTPS；仅本机地址允许 HTTP。"],
    token_required: ["Enter the management key for this CPA address.", "请输入这个 CPA 地址对应的管理密钥。"],
    invalid_token: ["Invalid management key format.", "管理密钥格式无效。"],
    credential_save_failed: ["Cannot save to the system credential store.", "无法保存到系统凭据库。"],
    credential_delete_failed: ["Cannot remove the saved connection.", "无法清除已保存的连接。"],
    connection_failed: ["Cannot reach CPA. Check its address, service and proxy.", "无法连接 CPA，请检查地址、服务与代理。"],
    management_http_401: ["Management key rejected (401).", "管理密钥被拒绝（401）。"],
    management_http_403: ["CPA denied management access (403).", "CPA 拒绝管理访问（403）。"],
    management_http_404: ["Management API unavailable. Check the URL and enable CPA management.", "未找到管理接口，请检查地址并启用 CPA 管理。"],
    account_disabled: ["Account disabled in CPA; not queried.", "账号已在 CPA 停用，未查询。"],
    unsupported_provider: ["This account provider is not supported yet.", "暂不支持此账号类型。"],
    missing_auth_index: ["CPA did not return an auth index. Update CPA and retry.", "CPA 未返回账号索引，请更新 CPA 后重试。"],
    missing_project_id: ["Antigravity did not return a project ID. Reconnect the account in CPA.", "Antigravity 未返回项目 ID，请在 CPA 重新连接账号。"],
    no_quota_data: ["The provider returned no quota data; remaining quota is unknown.", "服务商未返回配额数据，剩余额度未知。"],
    refresh_budget_exceeded: ["Not queried within this refresh budget. Retry with a smaller CPA pool.", "本轮查询超时，尚未查询此账号；请缩小 CPA 账号池后重试。"],
    invalid_response: ["Unexpected provider response; retry or update CPA.", "接口响应格式异常，请重试或更新 CPA。"],
    invalid_inventory: ["CPA returned an invalid account list.", "CPA 返回的账号列表格式异常。"],
    response_too_large: ["CPA response exceeds the 4 MB limit.", "CPA 响应超过 4 MB 上限。"],
    worker_failed: ["Quota refresh failed. Please retry.", "配额刷新失败，请重试。"],
  };
  if (messages[code]) return messages[code][zh ? 1 : 0];
  if (code.startsWith("grok_billing_unavailable")) return zh
    ? "Grok 部分账单数据不可用；付费账号可能不提供此接口，未知额度不会显示为无限。"
    : "Some Grok billing data is unavailable; paid accounts may not expose it. Unknown quota is not unlimited.";
  const status = code.match(/^(?:provider|management)_http_(\d+)$/)?.[1];
  if (status) return zh ? `接口返回 ${status}，请检查账号授权或稍后重试。` : `API returned ${status}. Check account authorization or retry later.`;
  return messages.connection_failed[zh ? 1 : 0];
}

export function CpaSettings({ onSaved }: { onSaved?: () => void }) {
  const { t, locale } = useI18n();
  const [connection, setConnection] = useState<Connection | null>(null);
  const [baseUrl, setBaseUrl] = useState("http://127.0.0.1:8317");
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [confirmDisconnect, setConfirmDisconnect] = useState(false);
  const load = useCallback(async () => {
    setError("");
    try {
      const config = await invoke<Connection>("cpa_config");
      setConnection(config); setBaseUrl(config.baseUrl);
    } catch (e) { setError(typeof e === "string" ? e : "connection_failed"); }
  }, []);
  useEffect(() => { void load(); }, [load]);

  async function save(event: React.FormEvent) {
    event.preventDefault();
    if (busy) return;
    setBusy(true); setError(""); setMessage("");
    try {
      const config = await invoke<Connection>("save_cpa_config", { baseUrl, token });
      setConnection(config); setBaseUrl(config.baseUrl); setToken("");
      setMessage(t("Connection verified and saved.", "连接已验证并保存。"));
      onSaved?.();
    } catch (e) { setError(typeof e === "string" ? e : "connection_failed"); }
    finally { setBusy(false); }
  }
  async function disconnect() {
    setBusy(true); setError(""); setMessage("");
    try {
      await invoke("disconnect_cpa");
      setToken(""); setConnection({ baseUrl, configured: false }); setConfirmDisconnect(false);
      setMessage(t("Saved connection removed. CPA accounts are unchanged.", "已清除保存的连接，CPA 账号不受影响。"));
      onSaved?.();
    } catch (e) { setError(typeof e === "string" ? e : "connection_failed"); }
    finally { setBusy(false); }
  }
  return <section className="cpa-settings" aria-label={t("CPA quota connection", "CPA 配额连接")}>
    <div className="cpa-heading"><strong>{t("CPA quota plugin", "CPA 配额插件")}</strong>
      <span>{connection?.configured ? t("Connected", "已配置") : t("Not connected", "未配置")}</span></div>
    <p>{t("Codex · Antigravity · Grok. Click CLIProxyAPI in the widget to expand usage and reset times.", "Codex · Antigravity · Grok。在悬浮窗点击 CLIProxyAPI，展开用量与重置时间。")}</p>
    <form onSubmit={(event) => void save(event)}>
      <fieldset disabled={busy || !connection}>
        <label>{t("CPA base URL", "CPA 基础地址")}<input type="url" required value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="http://127.0.0.1:8317" autoComplete="off" /></label>
        <label>{t("Management key", "管理密钥")}<input type="password" value={token} onChange={(e) => setToken(e.target.value)} autoComplete="new-password" placeholder={connection?.configured ? t("Leave blank to keep the key for this address", "留空保留此地址的密钥") : t("CPA management key, not an API key", "CPA 管理密钥，非调用 API Key")} /></label>
        <p>{t("Stored in the system credential store. OAuth files stay in CPA. Remote CPA connections use the local proxy; configure CPA's own outbound proxy for provider requests.", "密钥保存在系统凭据库，OAuth 文件留在 CPA。连接远程 CPA 使用本机代理；服务商查询走 CPA 自身配置的出站代理。")}</p>
        <div className="cpa-actions"><button className="btn primary" type="submit">{busy ? t("Verifying…", "正在验证…") : t("Test & save", "测试并保存")}</button>
          {connection?.configured && <button className="btn" type="button" onClick={() => setConfirmDisconnect(true)}>{t("Disconnect", "断开连接")}</button>}</div>
      </fieldset>
    </form>
    {!connection && !error && <p role="status">{t("Loading connection…", "正在加载连接…")}</p>}
    {!connection && error && <button className="btn small" onClick={() => void load()}>{t("Retry", "重试")}</button>}
    {confirmDisconnect && <div className="cpa-confirm"><p>{t("Remove the saved CPA connection and management key?", "清除已保存的 CPA 连接和管理密钥？")}</p><div className="cpa-actions"><button className="btn" disabled={busy} onClick={() => void disconnect()}>{t("Confirm disconnect", "确认断开")}</button><button className="btn" disabled={busy} onClick={() => setConfirmDisconnect(false)}>{t("Cancel", "取消")}</button></div></div>}
    {error && <p className="cpa-error" role="alert">{explain(error, locale === "zh-CN")}</p>}
    {message && <p role="status">{message}</p>}
  </section>;
}

export default function CpaQuota() {
  const { t, locale } = useI18n();
  const [snapshot, setSnapshot] = useState<QuotaSnapshot | null>(null);
  const [error, setError] = useState("");
  const [settings, setSettings] = useState(false);
  const [provider, setProvider] = useState("all");
  const [now, setNow] = useState(Date.now());
  const inFlight = useRef(false);
  const mounted = useRef(true);
  const refresh = useCallback(async (force = false) => {
    if (inFlight.current) return;
    inFlight.current = true;
    try {
      const value = await invoke<QuotaSnapshot>("cpa_quota", { force });
      if (mounted.current) { setSnapshot(value); setError(""); }
    } catch (e) { if (mounted.current) setError(typeof e === "string" ? e : "connection_failed"); }
    finally { inFlight.current = false; }
  }, []);
  useEffect(() => {
    mounted.current = true;
    void refresh();
    const timer = window.setInterval(() => { setNow(Date.now()); void refresh(); }, 2000);
    return () => { mounted.current = false; window.clearInterval(timer); };
  }, [refresh]);

  function resetLabel(window: QuotaWindow) {
    const at = window.resetAt ? new Date(window.resetAt).getTime() : NaN;
    if (!Number.isFinite(at)) return t("Reset time unavailable", "未提供重置时间");
    const minutes = Math.ceil((at - now) / 60000);
    const date = new Date(at).toLocaleString(locale, { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
    if (minutes <= 0) return `${date} · ${t("Due; refresh to confirm", "已到时间，刷新确认")}`;
    const duration = minutes >= 1440 ? `${Math.floor(minutes / 1440)}${t("d", "天")} ${Math.floor(minutes % 1440 / 60)}${t("h", "小时")}` : minutes >= 60 ? `${Math.floor(minutes / 60)}${t("h", "小时")} ${minutes % 60}${t("m", "分")}` : `${minutes}${t("m", "分")}`;
    return `${date} · ${duration}${t(" left", "后")}`;
  }
  function label(value: string) {
    return value.replace("Codex · shared", t("Codex · shared", "Codex · 共享额度"))
      .replace("Code review", t("Code review", "代码审查"))
      .replace("Antigravity · quota", t("Antigravity · quota", "Antigravity · 配额"))
      .replace("Grok · shared credits", t("Grok · shared credits", "Grok · 共享额度"))
      .replace("Grok · monthly spending cap", t("Grok · monthly spending cap", "Grok · 月度消费上限"));
  }
  const accounts = snapshot?.accounts.filter((a) => provider === "all" || a.provider === provider) ?? [];
  const stale = !!snapshot?.updatedAt && (!!snapshot.error || now - Date.parse(snapshot.updatedAt) > 180000);
  return <section className="cpa-quota" aria-label={t("CPA usage and reset details", "CPA 用量和重置时间详情")}>
    <div className="cpa-heading"><strong>{t("Usage & resets", "用量与重置")}</strong><div className="cpa-actions">
      <button className="icon-btn" onClick={() => setSettings(!settings)} aria-expanded={settings} aria-label={t("Configure CPA connection", "配置 CPA 连接")}><Settings2 size={14} /></button>
      <button className="icon-btn" disabled={snapshot?.refreshing} onClick={() => void refresh(true)} aria-label={t("Refresh quota", "刷新配额")}><RefreshCw size={14} className={snapshot?.refreshing ? "cpa-spinning" : ""} /></button>
    </div></div>
    {(settings || snapshot?.configured === false) && <CpaSettings onSaved={() => { setSnapshot(null); setSettings(false); void refresh(true); }} />}
    {(!snapshot || snapshot.refreshing) && <p role="status">{t("Fetching account quotas…", "正在查询账号配额…")}</p>}
    {(error || snapshot?.error) && <p className="cpa-error" role="alert">{explain(error || snapshot?.error, locale === "zh-CN")}</p>}
    {snapshot?.updatedAt && <p className={stale ? "cpa-error" : "cpa-updated"}>{stale ? t("Cached · ", "缓存数据 · ") : t("Updated · ", "更新于 · ")}{new Date(snapshot.updatedAt).toLocaleTimeString(locale)}{snapshot.refreshing ? t(" · refreshing", " · 刷新中") : ""}</p>}
    {!!snapshot?.accounts.length && <div className="cpa-filters" role="group" aria-label={t("Filter account provider", "筛选账号类型")}>
      {[["all", t("All", "全部")], ["codex", "Codex"], ["antigravity", "Antigravity"], ["grok", "Grok"]].map(([value, name]) => <button key={value} aria-pressed={provider === value} onClick={() => setProvider(value)}>{name}</button>)}
    </div>}
    {snapshot?.configured && !snapshot.refreshing && !snapshot.error && !error && accounts.length === 0 && <p>{t("No accounts in this view. Connect accounts in CPA, then refresh.", "此视图暂无账号，请在 CPA 连接账号后刷新。")}</p>}
    <div className="cpa-accounts">{accounts.map((account) => <details key={account.id} className="cpa-account" open>
      <summary><span><strong>{account.name || account.id}</strong><small>{account.provider === "grok" ? "Grok" : account.provider} {account.plan && `· ${account.plan}`}</small></span><ChevronDown size={14} /></summary>
      {account.error && <p className="cpa-error">{explain(account.error, locale === "zh-CN")}</p>}
      {account.windows.map((window, index) => <div className="cpa-window" key={`${window.label}-${index}`}>
        <div className="cpa-window-heading"><span>{label(window.label)}{window.windowSeconds ? ` · ${window.windowSeconds / 3600}${t("h", "小时")}` : ""}</span><strong>{window.remainingPercent === null ? "—" : `${Number(window.remainingPercent.toFixed(1))}%`}</strong></div>
        <div className="cpa-meter" role="meter" aria-label={`${label(window.label)} ${t("remaining", "剩余")}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={window.remainingPercent ?? undefined} aria-valuetext={window.remainingPercent === null ? t("Unknown", "未知") : `${window.remainingPercent}%`}>
          {window.remainingPercent !== null && <span className={window.remainingPercent <= 10 ? "low" : window.remainingPercent <= 25 ? "warn" : ""} style={{ width: `${window.remainingPercent}%` }} />}
        </div>
        <small>{window.remainingPercent === null ? t("Remaining unknown", "剩余未知") : t(`Used ${Number((100 - window.remainingPercent).toFixed(1))}%`, `已用 ${Number((100 - window.remainingPercent).toFixed(1))}%`)} · {window.kind === "billing" ? t("Billing rollover", "账期更新") : t("Resets", "重置")}</small>
        <time dateTime={window.resetAt ?? undefined}>{resetLabel(window)}</time>
      </div>)}
    </details>)}</div>
    {snapshot?.configured && <p className="cpa-footnote">{t("Refreshes every 2 min while open. Shared quotas are shown once per account. Grok billing rollover is separate from model rate limits.", "展开时每 2 分钟刷新。同一账号的共享额度仅显示一次；Grok 账期更新与模型限流重置分别标注。")}</p>}
  </section>;
}
