use crate::model::{BotChannel, BotPlatform, BotTargetKind, PopoConfig};
use crate::state::{
    lk, BotConnectionHandle, BotConnectionState, BotConnectionStatus, Inner, Shared,
};
use crate::vault;
use serde::Serialize;
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::header::USER_AGENT;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{client_tls, Message, WebSocket};
use url::Url;

const QQ_TOKEN_URL: &str = "https://bots.qq.com/app/getAppAccessToken";
const QQ_API_BASE: &str = "https://api.sgroup.qq.com";
const QQ_GATEWAY_PATH: &str = "/gateway";
const QQ_INTENTS: u64 = (1 << 25) | (1 << 26) | (1 << 30) | (1 << 12);
const USER_AGENT_VALUE: &str = concat!("Mosaic/", env!("CARGO_PKG_VERSION"));

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInfo {
    pub id: String,
    pub name: String,
    pub configured: bool,
    pub note: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BotChannelView {
    pub id: String,
    pub name: String,
    pub platform: BotPlatform,
    pub app_id: String,
    pub enabled: bool,
    pub target_kind: BotTargetKind,
    pub target: String,
    pub created_at: String,
    pub secret_configured: bool,
    pub status: BotConnectionStatus,
    pub status_detail: String,
}

pub fn bot_views(state: &Inner, channels: &[BotChannel]) -> Vec<BotChannelView> {
    let connections = lk(&state.bot_connections);
    channels
        .iter()
        .map(|channel| {
            let runtime = connections.get(&channel.id).map(|handle| lk(&handle.state).clone());
            let (status, status_detail) = match runtime {
                Some(runtime) => (runtime.status, runtime.detail),
                None if channel.enabled => (
                    BotConnectionStatus::Connecting,
                    "等待本地连接进程启动".into(),
                ),
                None => (BotConnectionStatus::Stopped, "已停止并收纳".into()),
            };
            BotChannelView {
                id: channel.id.clone(),
                name: channel.name.clone(),
                platform: channel.platform,
                app_id: channel.app_id.clone(),
                enabled: channel.enabled,
                target_kind: channel.target_kind,
                target: channel.target.clone(),
                created_at: channel.created_at.clone(),
                secret_configured: vault::contains(&vault::bot_channel_key(&channel.id)),
                status,
                status_detail,
            }
        })
        .collect()
}

pub fn list(popo: &PopoConfig, bots: &[BotChannel]) -> Vec<ChannelInfo> {
    let popo_note = match (&popo.target, popo.enabled) {
        (Some(target), true) => format!("发送到 {} ({})", target.alias, target.ip),
        (Some(target), false) => format!("已选 {}（已收纳）", target.alias),
        (None, _) => "已收纳；启用后可扫描局域网 PoPo 设备".into(),
    };
    let mut items = vec![
        ChannelInfo {
            id: "notify".into(),
            name: "系统通知".into(),
            configured: true,
            note: "本地通知中心".into(),
        },
        ChannelInfo {
            id: "popo".into(),
            name: "PoPo".into(),
            configured: popo.enabled && popo.target.is_some(),
            note: popo_note,
        },
    ];
    items.extend(bots.iter().map(|bot| ChannelInfo {
        id: format!("bot:{}", bot.id),
        name: bot.name.clone(),
        configured: bot.enabled
            && bot.platform == BotPlatform::Qq
            && vault::contains(&vault::bot_channel_key(&bot.id)),
        note: format!("{} · 本地 WebSocket", platform_name(bot.platform)),
    }));
    items
}

pub fn platform_name(platform: BotPlatform) -> &'static str {
    match platform {
        BotPlatform::Qq => "QQ",
        BotPlatform::Telegram => "Telegram（旧版未适配）",
        BotPlatform::Discord => "Discord（旧版未适配）",
        BotPlatform::Slack => "Slack（旧版未适配）",
        BotPlatform::Feishu => "飞书（旧版未适配）",
        BotPlatform::DingTalk => "钉钉（旧版未适配）",
        BotPlatform::WeCom => "企业微信（旧版未适配）",
    }
}

fn is_route_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub fn validate_channel(channel: &mut BotChannel, secret: Option<&str>) -> Result<(), String> {
    channel.name = channel.name.trim().to_string();
    channel.app_id = channel.app_id.trim().to_string();
    channel.target = channel.target.trim().to_string();
    if channel.platform != BotPlatform::Qq {
        return Err("该平台的桌面端协议尚未适配，不能作为已支持机器人保存".into());
    }
    if channel.name.is_empty() || channel.name.chars().count() > 60 {
        return Err("机器人名称不能为空且不能超过 60 个字符".into());
    }
    if channel.app_id.len() < 4
        || channel.app_id.len() > 80
        || !channel.app_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err("请填写有效的 QQ 开放平台 AppID".into());
    }
    if !is_route_component(&channel.target) {
        return Err("请填写有效的群 openid 或用户 openid".into());
    }
    if let Some(value) = secret {
        let value = value.trim();
        if value.len() < 8 || value.len() > 512 || value.chars().any(char::is_whitespace) {
            return Err("QQ AppSecret 格式无效".into());
        }
    }
    Ok(())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(8))
        .timeout_write(Duration::from_secs(8))
        .redirects(0)
        .build()
}

fn request_token(channel: &BotChannel, secret: &str) -> Result<String, String> {
    let response = agent()
        .post(QQ_TOKEN_URL)
        .set("Content-Type", "application/json")
        .set("User-Agent", USER_AGENT_VALUE)
        .send_json(json!({ "appId": channel.app_id, "clientSecret": secret }))
        .map_err(|error| match error {
            ureq::Error::Status(code, _) => format!("QQ 凭据校验失败（HTTP {}）", code),
            _ => "无法连接 QQ 令牌服务".into(),
        })?;
    let payload: Value = response
        .into_json()
        .map_err(|_| "QQ 令牌服务返回了无法识别的响应".to_string())?;
    payload
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "QQ 凭据无效：响应中没有 access_token".to_string())
}

fn request_gateway(token: &str) -> Result<String, String> {
    let response = agent()
        .get(&format!("{}{}", QQ_API_BASE, QQ_GATEWAY_PATH))
        .set("Authorization", &format!("QQBot {}", token))
        .set("User-Agent", USER_AGENT_VALUE)
        .call()
        .map_err(|error| match error {
            ureq::Error::Status(code, _) => format!("QQ 网关拒绝连接（HTTP {}）", code),
            _ => "无法获取 QQ WebSocket 网关".into(),
        })?;
    let payload: Value = response
        .into_json()
        .map_err(|_| "QQ 网关返回了无法识别的响应".to_string())?;
    let gateway = payload
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "QQ 网关响应中没有 WebSocket 地址".to_string())?;
    validate_gateway_url(gateway)?;
    Ok(gateway.to_string())
}

fn validate_gateway_url(gateway: &str) -> Result<(), String> {
    let url = Url::parse(gateway).map_err(|_| "QQ 返回了无效的 WebSocket 地址".to_string())?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if url.scheme() != "wss" || !(host == "qq.com" || host.ends_with(".qq.com")) {
        return Err("QQ 返回的网关不在受信任的 qq.com WSS 域名下".into());
    }
    Ok(())
}

fn set_read_timeout(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> Result<(), String> {
    let timeout = Some(Duration::from_millis(500));
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => stream.set_read_timeout(timeout),
        MaybeTlsStream::NativeTls(stream) => stream.get_mut().set_read_timeout(timeout),
        _ => return Err("当前 TLS 后端无法设置 QQ 连接超时".into()),
    }
    .map_err(|_| "无法设置 QQ WebSocket 读取超时".to_string())
}

fn set_status(
    runtime: &Arc<Mutex<BotConnectionState>>,
    app: &AppHandle,
    status: BotConnectionStatus,
    detail: impl Into<String>,
) {
    *lk(runtime) = BotConnectionState {
        status,
        detail: detail.into(),
    };
    let _ = app.emit("mosaic:changed", ());
}

fn interruptible_sleep(kill: &AtomicBool, duration: Duration) {
    let until = Instant::now() + duration;
    while !kill.load(Ordering::Relaxed) && Instant::now() < until {
        thread::sleep(Duration::from_millis(100));
    }
}

fn write_json(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    payload: Value,
) -> Result<(), String> {
    socket
        .send(Message::Text(payload.to_string().into()))
        .map_err(|_| "QQ WebSocket 写入失败".to_string())
}

fn connect_gateway(
    request: tungstenite::handshake::client::Request,
    gateway: &Url,
) -> Result<WebSocket<MaybeTlsStream<TcpStream>>, String> {
    let host = gateway
        .host_str()
        .ok_or_else(|| "QQ WebSocket 地址缺少主机名".to_string())?;
    let port = gateway.port_or_known_default().unwrap_or(443);
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|_| "无法解析 QQ WebSocket 主机".to_string())?;
    let mut stream = None;
    for address in addresses.take(2) {
        if let Ok(candidate) = TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
            stream = Some(candidate);
            break;
        }
    }
    let stream = stream.ok_or_else(|| "QQ WebSocket 连接超时".to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(8)))
        .map_err(|_| "无法设置 QQ TLS 握手超时".to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(8)))
        .map_err(|_| "无法设置 QQ TLS 写入超时".to_string())?;
    client_tls(request, stream)
        .map(|(socket, _)| socket)
        .map_err(|_| "QQ WebSocket TLS 或协议握手失败".to_string())
}

fn connect_once(
    channel: &BotChannel,
    secret: &str,
    kill: &AtomicBool,
    runtime: &Arc<Mutex<BotConnectionState>>,
    app: &AppHandle,
) -> Result<(), String> {
    set_status(runtime, app, BotConnectionStatus::Connecting, "正在校验 QQ 凭据");
    let token = request_token(channel, secret)?;
    if kill.load(Ordering::Relaxed) {
        return Ok(());
    }
    set_status(runtime, app, BotConnectionStatus::Connecting, "正在获取 WebSocket 网关");
    let gateway = request_gateway(&token)?;
    if kill.load(Ordering::Relaxed) {
        return Ok(());
    }
    let gateway_url = Url::parse(&gateway)
        .map_err(|_| "QQ 返回了无效的 WebSocket 地址".to_string())?;
    let mut request = gateway
        .as_str()
        .into_client_request()
        .map_err(|_| "无法创建 QQ WebSocket 请求".to_string())?;
    request
        .headers_mut()
        .insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    let mut socket = connect_gateway(request, &gateway_url)?;
    set_read_timeout(&mut socket)?;

    let mut last_sequence: Option<i64> = None;
    let mut heartbeat_every: Option<Duration> = None;
    let mut heartbeat_due: Option<Instant> = None;
    let mut ready = false;

    while !kill.load(Ordering::Relaxed) {
        match socket.read() {
            Ok(Message::Text(raw)) => {
                let payload: Value = serde_json::from_str(raw.as_ref())
                    .map_err(|_| "QQ WebSocket 返回了无效 JSON".to_string())?;
                if let Some(sequence) = payload.get("s").and_then(Value::as_i64) {
                    last_sequence = Some(sequence);
                }
                match payload.get("op").and_then(Value::as_i64) {
                    Some(10) => {
                        let interval_ms = payload
                            .pointer("/d/heartbeat_interval")
                            .and_then(Value::as_u64)
                            .ok_or_else(|| "QQ Hello 缺少心跳间隔".to_string())?;
                        let interval = Duration::from_millis((interval_ms * 8 / 10).max(1000));
                        heartbeat_every = Some(interval);
                        heartbeat_due = Some(Instant::now() + interval);
                        write_json(
                            &mut socket,
                            json!({
                                "op": 2,
                                "d": {
                                    "token": format!("QQBot {}", token),
                                    "intents": QQ_INTENTS,
                                    "shard": [0, 1],
                                    "properties": {
                                        "$os": "windows",
                                        "$browser": "mosaic",
                                        "$device": "mosaic"
                                    }
                                }
                            }),
                        )?;
                    }
                    Some(0) => match payload.get("t").and_then(Value::as_str) {
                        Some("READY") => {
                            ready = true;
                            let bot_name = payload
                                .pointer("/d/user/username")
                                .and_then(Value::as_str)
                                .unwrap_or("QQ 机器人");
                            set_status(
                                runtime,
                                app,
                                BotConnectionStatus::Online,
                                format!("{} · WebSocket 在线", bot_name),
                            );
                        }
                        Some("RESUMED") => {
                            ready = true;
                            set_status(
                                runtime,
                                app,
                                BotConnectionStatus::Online,
                                "QQ WebSocket 会话已恢复",
                            );
                        }
                        _ => {}
                    },
                    Some(7) => return Err("QQ 要求重新连接".into()),
                    Some(9) => return Err("QQ WebSocket 会话失效".into()),
                    _ => {}
                }
            }
            Ok(Message::Ping(payload)) => {
                socket
                    .send(Message::Pong(payload))
                    .map_err(|_| "QQ WebSocket Pong 写入失败".to_string())?;
            }
            Ok(Message::Close(frame)) => {
                let reason = frame
                    .map(|value| format!("{} {}", value.code, value.reason))
                    .unwrap_or_else(|| "无关闭原因".into());
                return Err(format!("QQ WebSocket 已关闭：{}", reason));
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return Err("QQ WebSocket 连接中断".into()),
        }

        if let (Some(every), Some(due)) = (heartbeat_every, heartbeat_due) {
            if Instant::now() >= due {
                write_json(&mut socket, json!({ "op": 1, "d": last_sequence }))?;
                heartbeat_due = Some(Instant::now() + every);
            }
        }
    }

    let _ = socket.close(None);
    if ready {
        let _ = socket.flush();
    }
    Ok(())
}

fn worker(
    channel: BotChannel,
    secret: String,
    kill: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    runtime: Arc<Mutex<BotConnectionState>>,
    app: AppHandle,
) {
    let backoff = [2_u64, 5, 10, 30, 60];
    let mut attempt = 0_usize;
    while !kill.load(Ordering::Relaxed) {
        match connect_once(&channel, &secret, &kill, &runtime, &app) {
            Ok(()) if kill.load(Ordering::Relaxed) => break,
            Ok(()) => {}
            Err(error) => {
                let delay = backoff[attempt.min(backoff.len() - 1)];
                set_status(
                    &runtime,
                    &app,
                    BotConnectionStatus::Error,
                    format!("{}；{} 秒后重连", error, delay),
                );
                interruptible_sleep(&kill, Duration::from_secs(delay));
                attempt = attempt.saturating_add(1);
            }
        }
    }
    set_status(
        &runtime,
        &app,
        BotConnectionStatus::Stopped,
        "本地 WebSocket 已终止",
    );
    done.store(true, Ordering::Release);
}

pub fn start(state: Shared, app: AppHandle, channel: BotChannel) -> Result<(), String> {
    if channel.platform != BotPlatform::Qq {
        return Err("这个旧版渠道没有可启动的桌面端适配器".into());
    }
    let secret = vault::get(&vault::bot_channel_key(&channel.id))
        .ok_or_else(|| "QQ AppSecret 缺失，请重新编辑保存".to_string())?;
    stop_and_wait(&state, &channel.id)?;
    let kill = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let runtime = Arc::new(Mutex::new(BotConnectionState {
        status: BotConnectionStatus::Connecting,
        detail: "等待本地连接进程启动".into(),
    }));
    lk(&state.bot_connections).insert(
        channel.id.clone(),
        BotConnectionHandle {
            kill: kill.clone(),
            done: done.clone(),
            state: runtime.clone(),
        },
    );
    thread::Builder::new()
        .name(format!("mosaic-qq-{}", channel.id))
        .spawn(move || worker(channel, secret, kill, done, runtime, app))
        .map_err(|_| "无法启动 QQ WebSocket 后台线程".to_string())?;
    Ok(())
}

pub fn start_enabled(state: Shared, app: AppHandle) {
    let channels: Vec<_> = lk(&state.db)
        .bot_channels
        .iter()
        .filter(|channel| channel.enabled && channel.platform == BotPlatform::Qq)
        .cloned()
        .collect();
    for channel in channels {
        let _ = start(state.clone(), app.clone(), channel);
    }
}

pub fn stop_and_wait(state: &Inner, id: &str) -> Result<(), String> {
    let handle = lk(&state.bot_connections).remove(id);
    let Some(handle) = handle else {
        return Ok(());
    };
    handle.kill.store(true, Ordering::Release);
    let until = Instant::now() + Duration::from_secs(12);
    while !handle.done.load(Ordering::Acquire) && Instant::now() < until {
        thread::sleep(Duration::from_millis(25));
    }
    if handle.done.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err("QQ WebSocket 已要求终止，但后台连接未在 12 秒内退出".into())
    }
}

pub fn stop_all(state: &Inner) {
    let ids: Vec<_> = lk(&state.bot_connections).keys().cloned().collect();
    for id in ids {
        let _ = stop_and_wait(state, &id);
    }
}

pub fn is_online(state: &Inner, id: &str) -> bool {
    lk(&state.bot_connections)
        .get(id)
        .map(|handle| matches!(lk(&handle.state).status, BotConnectionStatus::Online))
        .unwrap_or(false)
}

pub fn probe(channel: &BotChannel) -> Result<(), String> {
    let secret = vault::get(&vault::bot_channel_key(&channel.id))
        .ok_or_else(|| "QQ AppSecret 缺失，请重新编辑保存".to_string())?;
    let token = request_token(channel, &secret)?;
    request_gateway(&token).map(|_| ())
}

fn outbound_request(channel: &BotChannel, token: &str, text: &str) -> Result<(), String> {
    let path = match channel.target_kind {
        BotTargetKind::Group => format!("/v2/groups/{}/messages", channel.target),
        BotTargetKind::C2c => format!("/v2/users/{}/messages", channel.target),
    };
    let sequence = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        % 65_536;
    let body = json!({
        "content": text.chars().take(4000).collect::<String>(),
        "msg_type": 0,
        "msg_seq": sequence,
    });
    agent()
        .post(&format!("{}{}", QQ_API_BASE, path))
        .set("Authorization", &format!("QQBot {}", token))
        .set("Content-Type", "application/json")
        .set("User-Agent", USER_AGENT_VALUE)
        .send_json(body)
        .map_err(|error| match error {
            ureq::Error::Status(code, _) => format!(
                "QQ 主动消息被拒绝（HTTP {}）；请检查目标 openid、主动消息权限和额度",
                code
            ),
            _ => "QQ 主动消息发送失败或网络不可达".into(),
        })?;
    Ok(())
}

pub fn send(state: &Inner, channel: &BotChannel, text: &str) -> Result<(), String> {
    if !channel.enabled {
        return Err("机器人仍在收纳区，请先启用".into());
    }
    if !is_online(state, &channel.id) {
        return Err("QQ WebSocket 尚未在线，未发送任务消息".into());
    }
    let secret = vault::get(&vault::bot_channel_key(&channel.id))
        .ok_or_else(|| "QQ AppSecret 缺失，请重新编辑保存".to_string())?;
    let token = request_token(channel, &secret)?;
    outbound_request(channel, &token, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qq_channel() -> BotChannel {
        BotChannel {
            id: "qq-test".into(),
            name: "测试 QQ".into(),
            platform: BotPlatform::Qq,
            app_id: "102123456".into(),
            enabled: false,
            target_kind: BotTargetKind::Group,
            target: "group_openid-1".into(),
            created_at: String::new(),
        }
    }

    #[test]
    fn qq_config_rejects_webhooks_and_invalid_targets() {
        let mut channel = qq_channel();
        assert!(validate_channel(&mut channel, Some("secret-value")).is_ok());
        channel.target = "https://example.com/webhook".into();
        assert!(validate_channel(&mut channel, Some("secret-value")).is_err());
    }

    #[test]
    fn gateway_is_restricted_to_qq_wss() {
        assert!(validate_gateway_url("wss://gateway.qq.com/").is_ok());
        assert!(validate_gateway_url("ws://gateway.qq.com/").is_err());
        assert!(validate_gateway_url("wss://gateway.qq.com.example.org/").is_err());
    }

    #[test]
    fn identify_shape_matches_official_gateway_contract() {
        let payload = json!({
            "op": 2,
            "d": {
                "token": "QQBot token",
                "intents": QQ_INTENTS,
                "shard": [0, 1],
            }
        });
        assert_eq!(payload["op"], 2);
        assert_eq!(payload["d"]["intents"], 1_174_409_216_u64);
    }
}
