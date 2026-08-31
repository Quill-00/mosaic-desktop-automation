import { useMemo, useState } from "react";
import {
  Archive,
  Bot,
  Check,
  Download,
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

interface BotForm extends BotChannel {
  secret: string;
}

function newBot(platform: BotPlatform): BotForm {
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
    statusDetail: "未启用",
    secret: "",
  };
}

function platformInfo(platform: BotPlatform): PlatformInfo {
  return PLATFORMS.find((item) => item.id === platform) ?? PLATFORMS[0];
}

export default function Settings({ snap }: { snap: Snapshot }) {
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
    setMessage("正在扫描局域网…（约 10 秒）");
    try {
      const list = await api.popoScan();
      setPeers(list);
      setMessage(list.length ? `找到 ${list.length} 台 PoPo 设备` : "没找到 PoPo 设备，请确认接收端在同一局域网运行");
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setScanning(false);
    }
  }

  async function chooseTarget(peer: PopoPeer) {
    await api.savePopoConfig({ ...snap.popo, alias, target: peer, enabled: true });
    setMessage(`已选择 ${peer.alias} 为 PoPo 接收设备`);
  }

  async function togglePopo(enabled: boolean) {
    try {
      await api.savePopoConfig({ ...snap.popo, alias, enabled });
      setMessage(enabled ? "PoPo 已移到工作区" : "PoPo 已停止并收纳");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function testSendPopo() {
    try {
      await api.sendToPopo("来自 Mosaic 的测试消息");
      setMessage("PoPo 测试消息已发送，请在接收端确认。");
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  function openAddBot() {
    setBotStep("platform");
    setBotForm(newBot("qq"));
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
      setMessage("机器人已保存到未启用收纳区；先测试，再启用到任务发送列表。");
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
      setMessage(enabled ? `「${channel.name}」已启用` : `「${channel.name}」已停止并收纳`);
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function testChannel(channel: BotChannel) {
    setMessage(`正在验证「${channel.name}」的 QQ 官方连接…`);
    try {
      const result = await api.testBotChannel(channel.id);
      setMessage(`「${channel.name}」：${result}`);
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function confirmDeleteBot() {
    if (!deleteBot) return;
    try {
      await api.deleteBotChannel(deleteBot.id);
      setMessage(`「${deleteBot.name}」及其系统凭据已删除。`);
      setDeleteBot(null);
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function setWin(patch: Partial<WindowConfig>) {
    try {
      await api.saveWindowConfig({ ...snap.window, ...patch });
    } catch (error) {
      setMessage(`窗口设置失败：${errorMessage(error)}`);
    }
  }

  async function checkForUpdates() {
    setMessage("正在检查 GitHub 更新…");
    try {
      await api.checkForUpdates();
    } catch (error) {
      setMessage(`检查更新失败：${errorMessage(error)}`);
    }
  }

  const renderBotCard = (channel: BotChannel, shelved = false) => {
    const info = platformInfo(channel.platform);
    return (
      <article className={`channel-card${shelved ? " is-shelved" : ""}`} key={channel.id}>
        <div className="channel-card-head">
          <div className="channel-mark"><Bot size={17} /></div>
          <div>
            <strong>{channel.name}</strong>
            <span>{info.name} · {info.summary}</span>
          </div>
          <span className={`tag${channel.secretConfigured ? " success" : ""}`}>
            {channel.secretConfigured ? "凭据已存" : "缺少凭据"}
          </span>
        </div>
        {channel.platform === "qq" && (
          <>
            <div className="channel-target">{channel.targetKind === "group" ? "群 openid" : "用户 openid"}：{channel.target}</div>
            <div className={`channel-runtime is-${channel.status}`}>
              <span />{channel.statusDetail}
            </div>
          </>
        )}
        <div className="channel-actions">
          {shelved ? (
            <button className="btn small" onClick={() => void toggleBot(channel, true)} disabled={channel.platform !== "qq"}>
              <Check size={13} /> 启用
            </button>
          ) : (
            <button className="btn small" onClick={() => void toggleBot(channel, false)}>
              <Archive size={13} /> 收纳
            </button>
          )}
          <button className="btn small" onClick={() => void testChannel(channel)} disabled={channel.platform !== "qq"}>
            <Send size={13} /> {channel.enabled && channel.status === "online" ? "测试发送" : "验证连接"}
          </button>
          <button className="btn small" onClick={() => editChannel(channel)} disabled={channel.platform !== "qq"}>
            <Pencil size={13} /> 编辑
          </button>
          <button className="btn small danger" onClick={() => setDeleteBot(channel)} aria-label={`删除${channel.name}`}>
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
          <h2 className="view-title">发送渠道</h2>
          <p className="view-subtitle">任务完成后，把摘要推送到已启用的接收端</p>
        </div>
        <span className="muted" />
        <button className="btn primary" onClick={openAddBot}>
          <Plus size={14} /> 添加机器人
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
              <span>局域网文件接收端</span>
            </div>
            {snap.popo.target && <span className="tag info">{snap.popo.target.alias} · {snap.popo.target.ip}</span>}
            <button className="btn small" onClick={() => void togglePopo(false)}><Archive size={13} /> 收纳</button>
          </div>
          <label>
            本机显示名
            <input
              value={alias}
              onChange={(event) => setAlias(event.target.value)}
              onBlur={() => api.savePopoConfig({ ...snap.popo, alias })}
            />
          </label>
          <div className="channel-actions">
            <button className="btn" onClick={() => void scanPopo()} disabled={scanning}>
              <Wifi size={14} /> {scanning ? "扫描中…" : "扫描局域网"}
            </button>
            {snap.popo.target && <button className="btn" onClick={() => void testSendPopo()}><Send size={14} /> 测试发送</button>}
          </div>
          {peers.length > 0 && (
            <div className="rows">
              {peers.map((peer) => (
                <div className="row" key={peer.fingerprint}>
                  <div className="row-main">
                    <span className="row-name">{peer.alias}</span>
                    <span className="mono">{peer.ip}:{peer.port}</span>
                    <button className="btn small" onClick={() => void chooseTarget(peer)}><Check size={13} /> 选为接收设备</button>
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
            <span>未启用 · 已收纳渠道</span>
            <span className="inactive-count">{shelfCount}</span>
            <small>不会出现在任务发送列表</small>
          </button>
          {shelfOpen && (
            <div className="inactive-shelf-body channel-grid">
              {!snap.popo.enabled && (
                <article className="channel-card is-shelved">
                  <div className="channel-card-head">
                    <div className="channel-mark"><Download size={17} /></div>
                    <div><strong>PoPo</strong><span>局域网文件接收端</span></div>
                    <span className="tag">内置渠道</span>
                  </div>
                  <div className="channel-actions">
                    <button className="btn small" onClick={() => void togglePopo(true)}><Check size={13} /> 启用到工作区</button>
                  </div>
                </article>
              )}
              {inactiveBots.map((channel) => renderBotCard(channel, true))}
            </div>
          )}
        </div>
      )}

      <h2 className="view-title section-title">自动更新</h2>
      <section className="update-panel" aria-live="polite">
        <div>
          <strong>Mosaic {snap.update.currentVersion}</strong>
          <p>{snap.update.message}</p>
          <small>安装包只从 GitHub Releases 下载；校验通过后在下次启动时安装，不会中断当前工作。</small>
        </div>
        <span className={`tag${snap.update.state === "ready" || snap.update.state === "upToDate" ? " success" : ""}`}>
          {snap.update.state === "checking" ? "检查中" : snap.update.state === "ready" ? "等待重启" : snap.update.state === "upToDate" ? "已是最新" : snap.update.state === "error" ? "连接失败" : "自动检查"}
        </span>
        <button className="btn" onClick={() => void checkForUpdates()} disabled={snap.update.state === "checking"}>
          <RefreshCw size={14} className={snap.update.state === "checking" ? "spin" : ""} />
          {snap.update.state === "checking" ? "检查中…" : "重新检查"}
        </button>
      </section>

      <h2 className="view-title section-title">窗口与显示</h2>
      <div className="scan-tool">
        <label className="row-inline">
          <input type="checkbox" checked={snap.window.widget} onChange={(event) => void setWin({ widget: event.target.checked })} />
          桌面小组件（常驻桌面的悬浮窗）
        </label>
        <label className="row-inline">
          <input type="checkbox" checked={snap.window.edgeHide} onChange={(event) => void setWin({ edgeHide: event.target.checked })} />
          主窗口靠边自动隐藏（悬浮窗使用独立的左右边缘吸附机制）
        </label>
        <label className="row-inline">
          <input type="checkbox" checked={snap.window.minimizeToTray} onChange={(event) => void setWin({ minimizeToTray: event.target.checked })} />
          关闭时缩到托盘（而不是退出）
        </label>
      </div>

      {botForm !== null && (
        <div className="modal" onClick={() => setBotForm(null)}>
          <div className="sheet" role="dialog" aria-modal="true" aria-labelledby="bot-form-title" onClick={(event) => event.stopPropagation()}>
            <div className="sheet-head">
              <h3 id="bot-form-title">{botStep === "platform" ? "选择机器人平台" : botForm.id ? `编辑 ${botForm.name}` : `添加 ${platformInfo(botForm.platform).name} 机器人`}</h3>
              <button className="icon-btn" onClick={() => setBotForm(null)} aria-label="关闭"><X size={16} /></button>
            </div>
            {botStep === "platform" ? (
              <div className="bot-platform-grid">
                {PLATFORMS.map((platform) => (
                  <button
                    className={`bot-platform-tile${platform.available ? "" : " is-planned"}`}
                    key={platform.id}
                    disabled={!platform.available}
                    onClick={() => { if (platform.available) { setBotForm(newBot(platform.id)); setBotStep("form"); } }}
                  >
                    <Bot size={17} />
                    <strong>{platform.name}</strong>
                    <span>{platform.summary}</span>
                    {!platform.available && <small>按官方本地协议适配中</small>}
                  </button>
                ))}
              </div>
            ) : (
              <>
                <div className="community-security compact-warning">
                  <Bot size={17} />
                  <div><strong>{platformInfo(botForm.platform).name}</strong><p>{platformInfo(botForm.platform).note}</p></div>
                </div>
                <label>
                  显示名称
                  <input autoFocus value={botForm.name} onChange={(event) => setBotForm({ ...botForm, name: event.target.value })} placeholder="例如：运营通知群" />
                </label>
                <label>
                  QQ 开放平台 AppID
                  <input value={botForm.appId} onChange={(event) => setBotForm({ ...botForm, appId: event.target.value })} placeholder="例如：102123456" />
                </label>
                <label>
                  任务消息目标类型
                  <select value={botForm.targetKind} onChange={(event) => setBotForm({ ...botForm, targetKind: event.target.value as BotChannel["targetKind"] })}>
                    <option value="group">QQ群（group_openid）</option>
                    <option value="c2c">QQ 单聊（user openid）</option>
                  </select>
                </label>
                <label>
                  {botForm.targetKind === "group" ? "群 openid" : "用户 openid"}
                  <input value={botForm.target} onChange={(event) => setBotForm({ ...botForm, target: event.target.value })} placeholder={botForm.targetKind === "group" ? "QQ 开放平台 group_openid" : "QQ 开放平台用户 openid"} />
                </label>
                <label>
                  {platformInfo(botForm.platform).credential}
                  <input
                    type="password"
                    value={botForm.secret}
                    onChange={(event) => setBotForm({ ...botForm, secret: event.target.value })}
                    placeholder={botForm.id ? "留空保留已存凭据" : platformInfo(botForm.platform).placeholder}
                  />
                </label>
                <div className="scan-summary">AppSecret 存入 Windows 凭据管理器，不写入 db.json。Mosaic 只向腾讯官方接口发起 HTTPS / WSS 出站连接，不要求电脑暴露 Webhook。新增机器人默认收纳，验证后再启用。</div>
              </>
            )}
            <div className="sheet-actions">
              {botStep === "form" && !botForm.id && <button className="btn" onClick={() => setBotStep("platform")}>上一步</button>}
              <button className="btn" onClick={() => setBotForm(null)}>取消</button>
              {botStep === "form" && <button className="btn primary" disabled={savingBot} onClick={() => void saveChannel()}>{savingBot ? "保存中…" : "保存到收纳区"}</button>}
            </div>
          </div>
        </div>
      )}

      {deleteBot && (
        <div className="modal" onClick={() => setDeleteBot(null)}>
          <div className="sheet" role="alertdialog" aria-modal="true" aria-labelledby="delete-channel-title" onClick={(event) => event.stopPropagation()}>
            <h3 id="delete-channel-title">删除「{deleteBot.name}」</h3>
            <p className="scan-summary">将删除机器人配置和系统凭据，并清空任务对这个发送渠道的引用。</p>
            <div className="sheet-actions">
              <button className="btn" onClick={() => setDeleteBot(null)}>取消</button>
              <button className="btn danger" onClick={() => void confirmDeleteBot()}>确认删除</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
