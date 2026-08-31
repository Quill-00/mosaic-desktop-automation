import { useMemo, useState } from "react";
import {
  Archive,
  Bot,
  Check,
  Download,
  Globe2,
  Pencil,
  Plus,
  RefreshCw,
  Send,
  Trash2,
  Wifi,
  X,
} from "lucide-react";
import { api, errorMessage } from "../api";
import type { BotChannel, BotPlatform, PopoPeer, Snapshot, WindowConfig } from "../types";
import { useI18n } from "../i18n";
import type { Locale } from "../i18n";

interface PlatformInfo {
  id: BotPlatform;
  name: string;
  summary: string;
  credential: string;
  placeholder: string;
  note: string;
  available: boolean;
}

const PLATFORMS: PlatformInfo[] = [
  {
    id: "qq",
    name: "QQ 官方机器人",
    summary: "桌面 WebSocket 网关",
    credential: "AppSecret",
    placeholder: "QQ 开放平台 AppSecret",
    note: "Mosaic 在本机主动连接腾讯 WSS 网关，不需要公网回调地址；任务消息通过 QQ OpenAPI 发送。",
    available: true,
  },
  {
    id: "telegram",
    name: "Telegram",
    summary: "Long Poll（待适配）",
    credential: "Bot Token",
    placeholder: "123456:AA…",
    note: "桌面端应按 Bot API getUpdates 长轮询接入，本版未开放。",
    available: false,
  },
  {
    id: "discord",
    name: "Discord",
    summary: "Gateway WebSocket（待适配）",
    credential: "Bot Token",
    placeholder: "Discord Bot Token",
    note: "桌面端应连接 Discord Gateway，不以 Incoming Webhook 冒充机器人，本版未开放。",
    available: false,
  },
  {
    id: "slack",
    name: "Slack",
    summary: "Socket Mode（待适配）",
    credential: "App / Bot Token",
    placeholder: "xapp-… / xoxb-…",
    note: "桌面端应使用 Slack Socket Mode，本版未开放。",
    available: false,
  },
  {
    id: "feishu",
    name: "飞书",
    summary: "应用长连接（待适配）",
    credential: "App ID / App Secret",
    placeholder: "飞书应用凭据",
    note: "将按飞书应用事件长连接配置适配，本版未开放。",
    available: false,
  },
  {
    id: "dingTalk",
    name: "钉钉",
    summary: "Stream Mode（待适配）",
    credential: "Client ID / Client Secret",
    placeholder: "钉钉应用凭据",
    note: "将按钉钉 Stream Mode 适配，本版未开放。",
    available: false,
  },
  {
    id: "weCom",
    name: "企业微信",
    summary: "按官方应用协议适配中",
    credential: "应用凭据",
    placeholder: "企业微信应用凭据",
    note: "不复用 QQ 或其他平台的连接表单，本版未开放。",
    available: false,
  },
];

const PLATFORM_EN: Record<BotPlatform, Partial<PlatformInfo>> = {
  qq: { name: "QQ Official Bot", summary: "Desktop WebSocket gateway", placeholder: "QQ Open Platform AppSecret", note: "Mosaic connects to Tencent's WSS gateway from this device. No public callback URL is required, and task messages are sent through the QQ OpenAPI." },
  telegram: { summary: "Long polling (planned)", note: "The desktop client will use Bot API getUpdates long polling. This adapter is not available yet." },
  discord: { summary: "Gateway WebSocket (planned)", note: "The desktop client will connect to Discord Gateway instead of presenting an incoming webhook as a bot. This adapter is not available yet." },
  slack: { summary: "Socket Mode (planned)", note: "The desktop client will use Slack Socket Mode. This adapter is not available yet." },
  feishu: { name: "Feishu", summary: "Persistent app connection (planned)", placeholder: "Feishu app credentials", note: "This adapter will follow Feishu's persistent app event connection and is not available yet." },
  dingTalk: { name: "DingTalk", summary: "Stream Mode (planned)", placeholder: "DingTalk app credentials", note: "This adapter will follow DingTalk Stream Mode and is not available yet." },
  weCom: { name: "WeCom", summary: "Official app protocol (planned)", credential: "App credentials", placeholder: "WeCom app credentials", note: "This platform will have its own official connection form rather than reusing QQ settings. It is not available yet." },
};

interface BotForm extends BotChannel {
  secret: string;
}

function newBot(platform: BotPlatform, locale: Locale): BotForm {
  return {
    id: "",
    name: "",
    platform,
    appId: "",
    enabled: false,
    targetKind: "group",
    target: "",
    createdAt: "",
    secretConfigured: false,
    status: "stopped",
    statusDetail: locale === "zh-CN" ? "未启用" : "Disabled",
    secret: "",
  };
}

function platformInfo(platform: BotPlatform, locale: Locale): PlatformInfo {
  const base = PLATFORMS.find((item) => item.id === platform) ?? PLATFORMS[0];
  return locale === "zh-CN" ? base : { ...base, ...PLATFORM_EN[base.id] };
}

export default function Settings({ snap }: { snap: Snapshot }) {
  const { locale, setLocale, t } = useI18n();
  const [peers, setPeers] = useState<PopoPeer[]>([]);
  const [scanning, setScanning] = useState(false);
  const [message, setMessage] = useState("");
  const [alias, setAlias] = useState(snap.popo.alias);
  const [shelfOpen, setShelfOpen] = useState(false);
  const [botStep, setBotStep] = useState<"platform" | "form">("platform");
  const [botForm, setBotForm] = useState<BotForm | null>(null);
  const [savingBot, setSavingBot] = useState(false);
  const [deleteBot, setDeleteBot] = useState<BotChannel | null>(null);

  const activeBots = useMemo(() => snap.botChannels.filter((channel) => channel.enabled), [snap.botChannels]);
  const inactiveBots = useMemo(() => snap.botChannels.filter((channel) => !channel.enabled), [snap.botChannels]);
  const shelfCount = inactiveBots.length + (snap.popo.enabled ? 0 : 1);

  async function scanPopo() {
    setScanning(true);
    setMessage(t("Scanning the local network… (about 10 seconds)", "正在扫描局域网…（约 10 秒）"));
    try {
      const list = await api.popoScan();
      setPeers(list);
      setMessage(list.length ? t(`Found ${list.length} PoPo devices`, `找到 ${list.length} 台 PoPo 设备`) : t("No PoPo device found. Confirm that the receiver is running on the same local network.", "没找到 PoPo 设备，请确认接收端在同一局域网运行"));
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setScanning(false);
    }
  }

  async function chooseTarget(peer: PopoPeer) {
    await api.savePopoConfig({ ...snap.popo, alias, target: peer, enabled: true });
    setMessage(t(`Selected ${peer.alias} as the PoPo receiver`, `已选择 ${peer.alias} 为 PoPo 接收设备`));
  }

  async function togglePopo(enabled: boolean) {
    try {
      await api.savePopoConfig({ ...snap.popo, alias, enabled });
      setMessage(enabled ? t("PoPo was moved to the workspace", "PoPo 已移到工作区") : t("PoPo was stopped and shelved", "PoPo 已停止并收纳"));
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function testSendPopo() {
    try {
      await api.sendToPopo(t("Test message from Mosaic", "来自 Mosaic 的测试消息"));
      setMessage(t("The PoPo test message was sent. Confirm it on the receiving device.", "PoPo 测试消息已发送，请在接收端确认。"));
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  function openAddBot() {
    setBotStep("platform");
    setBotForm(newBot("qq", locale));
    setMessage("");
  }

  function editChannel(channel: BotChannel) {
    setBotStep("form");
    setBotForm({ ...channel, secret: "" });
    setMessage("");
  }

  async function saveChannel() {
    if (!botForm || savingBot) return;
    setSavingBot(true);
    setMessage("");
    try {
      const { secret, ...channel } = botForm;
      await api.saveBotChannel(channel, secret);
      setBotForm(null);
      setMessage(t("The bot was saved disabled. Test it before enabling it for task delivery.", "机器人已保存到未启用收纳区；先测试，再启用到任务发送列表。"));
      setShelfOpen(true);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setSavingBot(false);
    }
  }

  async function toggleBot(channel: BotChannel, enabled: boolean) {
    try {
      await api.setBotChannelEnabled(channel.id, enabled);
      setMessage(enabled ? t(`“${channel.name}” was enabled`, `「${channel.name}」已启用`) : t(`“${channel.name}” was stopped and shelved`, `「${channel.name}」已停止并收纳`));
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function testChannel(channel: BotChannel) {
    setMessage(t(`Verifying the official QQ connection for “${channel.name}”…`, `正在验证「${channel.name}」的 QQ 官方连接…`));
    try {
      const result = await api.testBotChannel(channel.id);
      setMessage(t(`“${channel.name}”: ${result}`, `「${channel.name}」：${result}`));
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function confirmDeleteBot() {
    if (!deleteBot) return;
    try {
      await api.deleteBotChannel(deleteBot.id);
      setMessage(t(`“${deleteBot.name}” and its system credential were deleted.`, `「${deleteBot.name}」及其系统凭据已删除。`));
      setDeleteBot(null);
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function setWin(patch: Partial<WindowConfig>) {
    try {
      await api.saveWindowConfig({ ...snap.window, ...patch });
    } catch (error) {
      setMessage(t(`Window setting failed: ${errorMessage(error)}`, `窗口设置失败：${errorMessage(error)}`));
    }
  }

  async function checkForUpdates() {
    setMessage(t("Checking GitHub for updates…", "正在检查 GitHub 更新…"));
    try {
      await api.checkForUpdates();
    } catch (error) {
      setMessage(t(`Update check failed: ${errorMessage(error)}`, `检查更新失败：${errorMessage(error)}`));
    }
  }

  const updateMessage = locale === "zh-CN"
    ? snap.update.message
    : snap.update.state === "checking"
      ? "Checking GitHub for updates…"
      : snap.update.state === "ready"
        ? `Mosaic ${snap.update.latestVersion ?? "update"} is verified and will install on the next launch.`
        : snap.update.state === "upToDate"
          ? "You are running the latest version."
          : snap.update.state === "error"
            ? "Unable to connect to GitHub. Check your network; the current version remains available."
            : "Mosaic checks for updates automatically after launch.";

  const renderBotCard = (channel: BotChannel, shelved = false) => {
    const info = platformInfo(channel.platform, locale);
    return (
      <article className={`channel-card${shelved ? " is-shelved" : ""}`} key={channel.id}>
        <div className="channel-card-head">
          <div className="channel-mark"><Bot size={17} /></div>
          <div>
            <strong>{channel.name}</strong>
            <span>{info.name} · {info.summary}</span>
          </div>
          <span className={`tag${channel.secretConfigured ? " success" : ""}`}>
            {channel.secretConfigured ? t("Credential saved", "凭据已存") : t("Credential missing", "缺少凭据")}
          </span>
        </div>
        {channel.platform === "qq" && (
          <>
            <div className="channel-target">{channel.targetKind === "group" ? t("Group openid", "群 openid") : t("User openid", "用户 openid")}: {channel.target}</div>
            <div className={`channel-runtime is-${channel.status}`}>
              <span />{locale === "zh-CN" ? channel.statusDetail : channel.status === "online" ? "Online" : channel.status === "connecting" ? "Connecting" : channel.status === "error" ? "Connection error" : "Disabled"}
            </div>
          </>
        )}
        <div className="channel-actions">
          {shelved ? (
            <button className="btn small" onClick={() => void toggleBot(channel, true)} disabled={channel.platform !== "qq"}>
              <Check size={13} /> {t("Enable", "启用")}
            </button>
          ) : (
            <button className="btn small" onClick={() => void toggleBot(channel, false)}>
              <Archive size={13} /> {t("Shelve", "收纳")}
            </button>
          )}
          <button className="btn small" onClick={() => void testChannel(channel)} disabled={channel.platform !== "qq"}>
            <Send size={13} /> {channel.enabled && channel.status === "online" ? t("Send test", "测试发送") : t("Verify connection", "验证连接")}
          </button>
          <button className="btn small" onClick={() => editChannel(channel)} disabled={channel.platform !== "qq"}>
            <Pencil size={13} /> {t("Edit", "编辑")}
          </button>
          <button className="btn small danger" onClick={() => setDeleteBot(channel)} aria-label={t(`Delete ${channel.name}`, `删除${channel.name}`)}>
            <Trash2 size={13} />
          </button>
        </div>
      </article>
    );
  };

  return (
    <div className="view">
      <div className="view-head">
        <div>
          <h2 className="view-title">{t("Settings", "设置")}</h2>
          <p className="view-subtitle">{t("Language, updates, windows, and delivery channels", "语言、更新、窗口与发送渠道")}</p>
        </div>
      </div>

      <h2 className="view-title section-title">{t("Language", "语言")}</h2>
      <section className="language-panel">
        <div>
          <Globe2 size={17} aria-hidden="true" />
          <div>
            <strong>{t("Interface language", "界面语言")}</strong>
            <small>{t("First launch uses Chinese in UTC+8 and English in every other timezone. Your manual choice is remembered.", "首次启动在 UTC+8 时区使用中文，其他时区使用英文；手动选择后会记住你的偏好。")}</small>
          </div>
        </div>
        <div className="language-options" role="group" aria-label={t("Interface language", "界面语言")}>
          <button className={locale === "en" ? "btn primary" : "btn"} onClick={() => setLocale("en")} aria-pressed={locale === "en"}>English</button>
          <button className={locale === "zh-CN" ? "btn primary" : "btn"} onClick={() => setLocale("zh-CN")} aria-pressed={locale === "zh-CN"}>中文</button>
        </div>
      </section>

      <div className="view-head">
        <div>
          <h2 className="view-title">{t("Delivery channels", "发送渠道")}</h2>
          <p className="view-subtitle">{t("Send task summaries to enabled receivers", "任务完成后，把摘要推送到已启用的接收端")}</p>
        </div>
        <span className="muted" />
        <button className="btn primary" onClick={openAddBot}>
          <Plus size={14} /> {t("Add bot", "添加机器人")}
        </button>
      </div>

      {message && <div className="community-message" role="status">{message}</div>}

      <div className="channel-grid">
        {activeBots.map((channel) => renderBotCard(channel))}
      </div>

      {snap.popo.enabled && (
        <section className="popo-panel">
          <div className="channel-card-head">
            <div className="channel-mark"><Download size={17} /></div>
            <div>
              <strong>PoPo</strong>
              <span>{t("Local-network file receiver", "局域网文件接收端")}</span>
            </div>
            {snap.popo.target && <span className="tag info">{snap.popo.target.alias} · {snap.popo.target.ip}</span>}
            <button className="btn small" onClick={() => void togglePopo(false)}><Archive size={13} /> {t("Shelve", "收纳")}</button>
          </div>
          <label>
            {t("Device display name", "本机显示名")}
            <input
              value={alias}
              onChange={(event) => setAlias(event.target.value)}
              onBlur={() => api.savePopoConfig({ ...snap.popo, alias })}
            />
          </label>
          <div className="channel-actions">
            <button className="btn" onClick={() => void scanPopo()} disabled={scanning}>
              <Wifi size={14} /> {scanning ? t("Scanning…", "扫描中…") : t("Scan local network", "扫描局域网")}
            </button>
            {snap.popo.target && <button className="btn" onClick={() => void testSendPopo()}><Send size={14} /> {t("Send test", "测试发送")}</button>}
          </div>
          {peers.length > 0 && (
            <div className="rows">
              {peers.map((peer) => (
                <div className="row" key={peer.fingerprint}>
                  <div className="row-main">
                    <span className="row-name">{peer.alias}</span>
                    <span className="mono">{peer.ip}:{peer.port}</span>
                    <button className="btn small" onClick={() => void chooseTarget(peer)}><Check size={13} /> {t("Use as receiver", "选为接收设备")}</button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>
      )}

      {shelfCount > 0 && (
        <div className="inactive-shelf channel-shelf">
          <button className="inactive-shelf-head" onClick={() => setShelfOpen((open) => !open)} aria-expanded={shelfOpen}>
            <Archive size={14} />
            <span>{t("Disabled · Shelved channels", "未启用 · 已收纳渠道")}</span>
            <span className="inactive-count">{shelfCount}</span>
            <small>{t("Hidden from task delivery lists", "不会出现在任务发送列表")}</small>
          </button>
          {shelfOpen && (
            <div className="inactive-shelf-body channel-grid">
              {!snap.popo.enabled && (
                <article className="channel-card is-shelved">
                  <div className="channel-card-head">
                    <div className="channel-mark"><Download size={17} /></div>
                    <div><strong>PoPo</strong><span>{t("Local-network file receiver", "局域网文件接收端")}</span></div>
                    <span className="tag">{t("Built-in channel", "内置渠道")}</span>
                  </div>
                  <div className="channel-actions">
                    <button className="btn small" onClick={() => void togglePopo(true)}><Check size={13} /> {t("Enable in workspace", "启用到工作区")}</button>
                  </div>
                </article>
              )}
              {inactiveBots.map((channel) => renderBotCard(channel, true))}
            </div>
          )}
        </div>
      )}

      <h2 className="view-title section-title">{t("Automatic updates", "自动更新")}</h2>
      <section className="update-panel" aria-live="polite">
        <div>
          <strong>Mosaic {snap.update.currentVersion}</strong>
          <p>{updateMessage}</p>
          <small>{t("Installers are downloaded only from GitHub Releases. Verified updates install on the next launch without interrupting current work.", "安装包只从 GitHub Releases 下载；校验通过后在下次启动时安装，不会中断当前工作。")}</small>
        </div>
        <span className={`tag${snap.update.state === "ready" || snap.update.state === "upToDate" ? " success" : ""}`}>
          {snap.update.state === "checking" ? t("Checking", "检查中") : snap.update.state === "ready" ? t("Restart pending", "等待重启") : snap.update.state === "upToDate" ? t("Up to date", "已是最新") : snap.update.state === "error" ? t("Connection failed", "连接失败") : t("Automatic", "自动检查")}
        </span>
        <button className="btn" onClick={() => void checkForUpdates()} disabled={snap.update.state === "checking"}>
          <RefreshCw size={14} className={snap.update.state === "checking" ? "spin" : ""} />
          {snap.update.state === "checking" ? t("Checking…", "检查中…") : t("Check again", "重新检查")}
        </button>
      </section>

      <h2 className="view-title section-title">{t("Windows & display", "窗口与显示")}</h2>
      <div className="scan-tool">
        <label className="row-inline">
          <input type="checkbox" checked={snap.window.widget} onChange={(event) => void setWin({ widget: event.target.checked })} />
          {t("Desktop widget (always-available floating window)", "桌面小组件（常驻桌面的悬浮窗）")}
        </label>
        <label className="row-inline">
          <input type="checkbox" checked={snap.window.edgeHide} onChange={(event) => void setWin({ edgeHide: event.target.checked })} />
          {t("Auto-hide the main window at screen edges (the widget has separate left/right snapping)", "主窗口靠边自动隐藏（悬浮窗使用独立的左右边缘吸附机制）")}
        </label>
        <label className="row-inline">
          <input type="checkbox" checked={snap.window.minimizeToTray} onChange={(event) => void setWin({ minimizeToTray: event.target.checked })} />
          {t("Minimize to tray when closed instead of quitting", "关闭时缩到托盘（而不是退出）")}
        </label>
      </div>

      {botForm !== null && (
        <div className="modal" onClick={() => setBotForm(null)}>
          <div className="sheet" role="dialog" aria-modal="true" aria-labelledby="bot-form-title" onClick={(event) => event.stopPropagation()}>
            <div className="sheet-head">
              <h3 id="bot-form-title">{botStep === "platform" ? t("Choose a bot platform", "选择机器人平台") : botForm.id ? t(`Edit ${botForm.name}`, `编辑 ${botForm.name}`) : t(`Add ${platformInfo(botForm.platform, locale).name} bot`, `添加 ${platformInfo(botForm.platform, locale).name} 机器人`)}</h3>
              <button className="icon-btn" onClick={() => setBotForm(null)} aria-label={t("Close", "关闭")}><X size={16} /></button>
            </div>
            {botStep === "platform" ? (
              <div className="bot-platform-grid">
                {PLATFORMS.map((platform) => {
                  const info = platformInfo(platform.id, locale);
                  return (
                  <button
                    className={`bot-platform-tile${platform.available ? "" : " is-planned"}`}
                    key={platform.id}
                    disabled={!platform.available}
                    onClick={() => { if (platform.available) { setBotForm(newBot(platform.id, locale)); setBotStep("form"); } }}
                  >
                    <Bot size={17} />
                    <strong>{info.name}</strong>
                    <span>{info.summary}</span>
                    {!platform.available && <small>{t("Official local adapter planned", "按官方本地协议适配中")}</small>}
                  </button>
                  );
                })}
              </div>
            ) : (
              <>
                <div className="community-security compact-warning">
                  <Bot size={17} />
                  <div><strong>{platformInfo(botForm.platform, locale).name}</strong><p>{platformInfo(botForm.platform, locale).note}</p></div>
                </div>
                <label>
                  {t("Display name", "显示名称")}
                  <input autoFocus value={botForm.name} onChange={(event) => setBotForm({ ...botForm, name: event.target.value })} placeholder={t("For example: Operations notifications", "例如：运营通知群")} />
                </label>
                <label>
                  {t("QQ Open Platform AppID", "QQ 开放平台 AppID")}
                  <input value={botForm.appId} onChange={(event) => setBotForm({ ...botForm, appId: event.target.value })} placeholder={t("For example: 102123456", "例如：102123456")} />
                </label>
                <label>
                  {t("Task message target", "任务消息目标类型")}
                  <select value={botForm.targetKind} onChange={(event) => setBotForm({ ...botForm, targetKind: event.target.value as BotChannel["targetKind"] })}>
                    <option value="group">{t("QQ group (group_openid)", "QQ群（group_openid）")}</option>
                    <option value="c2c">{t("QQ direct message (user openid)", "QQ 单聊（user openid）")}</option>
                  </select>
                </label>
                <label>
                  {botForm.targetKind === "group" ? t("Group openid", "群 openid") : t("User openid", "用户 openid")}
                  <input value={botForm.target} onChange={(event) => setBotForm({ ...botForm, target: event.target.value })} placeholder={botForm.targetKind === "group" ? t("QQ Open Platform group_openid", "QQ 开放平台 group_openid") : t("QQ Open Platform user openid", "QQ 开放平台用户 openid")} />
                </label>
                <label>
                  {platformInfo(botForm.platform, locale).credential}
                  <input
                    type="password"
                    value={botForm.secret}
                    onChange={(event) => setBotForm({ ...botForm, secret: event.target.value })}
                    placeholder={botForm.id ? t("Leave blank to keep the saved credential", "留空保留已存凭据") : platformInfo(botForm.platform, locale).placeholder}
                  />
                </label>
                <div className="scan-summary">{t("The AppSecret is stored in Windows Credential Manager, not db.json. Mosaic makes outbound HTTPS/WSS connections only to official Tencent endpoints and does not expose a webhook on your computer. New bots are saved disabled until verified.", "AppSecret 存入 Windows 凭据管理器，不写入 db.json。Mosaic 只向腾讯官方接口发起 HTTPS / WSS 出站连接，不要求电脑暴露 Webhook。新增机器人默认收纳，验证后再启用。")}</div>
              </>
            )}
            <div className="sheet-actions">
              {botStep === "form" && !botForm.id && <button className="btn" onClick={() => setBotStep("platform")}>{t("Back", "上一步")}</button>}
              <button className="btn" onClick={() => setBotForm(null)}>{t("Cancel", "取消")}</button>
              {botStep === "form" && <button className="btn primary" disabled={savingBot} onClick={() => void saveChannel()}>{savingBot ? t("Saving…", "保存中…") : t("Save disabled", "保存到收纳区")}</button>}
            </div>
          </div>
        </div>
      )}

      {deleteBot && (
        <div className="modal" onClick={() => setDeleteBot(null)}>
          <div className="sheet" role="alertdialog" aria-modal="true" aria-labelledby="delete-channel-title" onClick={(event) => event.stopPropagation()}>
            <h3 id="delete-channel-title">{t(`Delete “${deleteBot.name}”`, `删除「${deleteBot.name}」`)}</h3>
            <p className="scan-summary">{t("This deletes the bot configuration and system credential, and removes the channel from every task that references it.", "将删除机器人配置和系统凭据，并清空任务对这个发送渠道的引用。")}</p>
            <div className="sheet-actions">
              <button className="btn" onClick={() => setDeleteBot(null)}>{t("Cancel", "取消")}</button>
              <button className="btn danger" onClick={() => void confirmDeleteBot()}>{t("Confirm delete", "确认删除")}</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
