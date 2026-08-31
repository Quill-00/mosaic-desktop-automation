use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type Id = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    #[serde(default)]
    pub id: Id,
    pub nickname: String,
    pub command: String,
    #[serde(default)]
    pub kind: TaskKind,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    pub trigger: Trigger,
    #[serde(default = "default_display")]
    pub display: DisplayForm,
    #[serde(default = "default_lifecycle")]
    pub lifecycle: Lifecycle,
    /// Whether the item is enabled in the user's working library. Inactive
    /// items remain stored but are tucked into the collapsed library shelf.
    #[serde(default = "default_true")]
    pub active: bool,
    /// Runtime switch: scheduler/watch participation for scripts, live process
    /// state for resident plugins. Only meaningful while `active` is true.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Optional channel to push this task's summary to after a successful run
    /// (currently "popo").
    #[serde(default)]
    pub push_channel: Option<String>,
    /// Whether this task appears as a module on the dashboard.
    #[serde(default = "default_true")]
    pub on_dashboard: bool,
    /// Sort position on the dashboard (lower = earlier).
    #[serde(default)]
    pub order: i32,
    /// Dashboard module size in grid units (snapped, never free-pixel).
    #[serde(default = "default_one")]
    pub col_span: u8,
    #[serde(default = "default_one")]
    pub row_span: u8,
    /// Optional directory holding this task's file products (e.g. a crawler's
    /// download folder). When set, the card/timeline lists the most recent files
    /// here instead of parsing stdout — so file-producing scripts show products.
    #[serde(default)]
    pub output_dir: Option<String>,
    /// Canned input fed to the script's stdin (for CLI scripts that prompt, e.g.
    /// PowerShell `Read-Host` / Python `input()`). One answer per line.
    #[serde(default)]
    pub stdin: Option<String>,
    /// Provenance for tasks installed from a community registry. Community
    /// packages are always installed disabled and still execute with the
    /// current Windows user's permissions once explicitly enabled.
    #[serde(default)]
    pub community: Option<CommunityTaskSource>,
    #[serde(default)]
    pub created_at: String,
}

fn default_display() -> DisplayForm {
    DisplayForm::Card
}
fn default_lifecycle() -> Lifecycle {
    Lifecycle::Ephemeral
}
fn default_timeout() -> u64 {
    60
}

fn default_true() -> bool {
    true
}

fn default_one() -> u8 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Trigger {
    Manual,
    #[serde(rename_all = "camelCase")]
    Interval {
        // `alias` keeps loading older db.json that stored the snake_case name.
        #[serde(alias = "every_secs")]
        every_secs: u64,
    },
    Daily {
        at: String,
    },
    Weekly {
        /// Weekdays to run on, 0 = Sunday .. 6 = Saturday (matches JS getDay).
        #[serde(default)]
        days: Vec<u8>,
        at: String,
    },
    Monthly {
        /// Day of month 1..31 (clamped to the month's length).
        day: u8,
        at: String,
    },
    #[serde(rename_all = "camelCase")]
    Watch {
        path: String,
        #[serde(default = "star")]
        pattern: String,
        #[serde(default)]
        recursive: bool,
    },
}

fn star() -> String {
    "*".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayForm {
    Strip,
    Card,
    Metric,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Lifecycle {
    Ephemeral,
    Resident,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskKind {
    #[default]
    Script,
    Plugin,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    #[serde(default)]
    pub read_paths: Vec<String>,
    #[serde(default)]
    pub write_paths: Vec<String>,
    #[serde(default)]
    pub allow_hosts: Vec<String>,
    #[serde(default)]
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityTaskSource {
    pub package_id: String,
    pub version: String,
    pub registry_url: String,
    pub package_dir: String,
    pub sha256: String,
    pub author: String,
    pub runtime: PackageRuntime,
    pub risk: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageRuntime {
    Node,
    Python,
    PowerShell,
    Executable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryPackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub summary: String,
    #[serde(default)]
    pub description: String,
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub repository: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub kind: TaskKind,
    pub runtime: PackageRuntime,
    pub entry: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_lifecycle")]
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub permissions: Permissions,
    pub package_url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryDocument {
    pub schema_version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub packages: Vec<RegistryPackage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityConfig {
    #[serde(default)]
    pub sources: Vec<String>,
}

pub const OFFICIAL_COMMUNITY_SOURCE: &str = "https://raw.githubusercontent.com/Quill-00/mosaic-desktop-automation/main/community/registry.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub headline: String,
    #[serde(default)]
    pub count: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub at: Option<String>,
    /// "image" / "file" / source-defined; drives how the detail view renders it.
    #[serde(default)]
    pub kind: Option<String>,
    /// Absolute file path for file products, so the UI can preview them.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOutput {
    #[serde(default)]
    pub summary: Option<Summary>,
    #[serde(default)]
    pub card: Option<serde_json::Value>,
    #[serde(default)]
    pub items: Vec<DetailItem>,
    /// Checkpoint the script reports; fed back next run as MOSAIC_CURSOR.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResultState {
    #[serde(default)]
    pub summary: Option<Summary>,
    #[serde(default)]
    pub card: Option<serde_json::Value>,
    #[serde(default)]
    pub timeline: Vec<DetailItem>,
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Last checkpoint the script reported (fed back as MOSAIC_CURSOR).
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecStatus {
    Running,
    Ok,
    Failed,
    TimedOut,
    Killed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub id: Id,
    pub task_id: Id,
    pub nickname: String,
    pub started_at: String,
    #[serde(default)]
    pub finished_at: Option<String>,
    pub status: ExecStatus,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub trigger: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub item_count: usize,
    /// Products of this specific run, so the detail view can browse content per
    /// round. Stripped from the dashboard snapshot (fetched on demand via
    /// `exec_items`) to keep the snapshot light.
    #[serde(default)]
    pub items: Vec<DetailItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: Id,
    pub level: String,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub at: String,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub task_id: Option<Id>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Codex,
    OpenaiCompatible,
}

impl Default for ProviderKind {
    fn default() -> Self {
        ProviderKind::OpenaiCompatible
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthMode {
    ApiKey,
    DeviceCode,
}

impl Default for AuthMode {
    fn default() -> Self {
        AuthMode::ApiKey
    }
}

fn openid_scope() -> String {
    "openid profile".into()
}

/// An LLM provider. Mirrors Automata's AiProviderSettings: a Codex CLI provider
/// (auth handled by the `codex` CLI) or an OpenAI-compatible endpoint (API key in
/// the vault, or an OAuth device-code token). Secrets never live here — only in
/// the OS-backed credential vault, keyed by `provider:{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub id: Id,
    pub display_name: String,
    #[serde(default)]
    pub kind: ProviderKind,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub auth_mode: AuthMode,
    #[serde(default)]
    pub device_auth_url: String,
    #[serde(default)]
    pub token_url: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default = "openid_scope")]
    pub scope: String,
}

fn default_alias() -> String {
    "Mosaic".into()
}

/// A PoPo / LocalSend peer found on the LAN (and chosen as the send target).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopoPeer {
    pub ip: String,
    pub port: u16,
    pub alias: String,
    pub fingerprint: String,
}

/// Config for sending data to PoPo (the user's own transfer app). Mosaic speaks
/// PoPo's wire protocol directly (LocalSend v2: discover on LAN, then
/// prepare-upload + upload) — no PoPo HTTP API, no crate coupling. Users download
/// PoPo separately to receive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopoConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_alias")]
    pub alias: String,
    /// Stable per-install identifier advertised to peers (generated on first use).
    #[serde(default)]
    pub fingerprint: String,
    /// The chosen PoPo device to send to.
    #[serde(default)]
    pub target: Option<PopoPeer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BotPlatform {
    Qq,
    // Legacy 0.2 development values are retained so an existing db.json can
    // still be read without discarding unrelated tasks. They are deliberately
    // not exposed as available desktop adapters until their official local
    // transport has been implemented.
    Telegram,
    Discord,
    Slack,
    Feishu,
    DingTalk,
    WeCom,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum BotTargetKind {
    #[default]
    Group,
    C2c,
}

/// A platform robot channel. Secrets are never serialized here; only public
/// application and routing metadata is persisted in db.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BotChannel {
    pub id: Id,
    pub name: String,
    pub platform: BotPlatform,
    /// Public QQ Open Platform application id. The AppSecret stays in the OS
    /// credential vault and is never serialized here.
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub target_kind: BotTargetKind,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub created_at: String,
}

impl Default for PopoConfig {
    fn default() -> Self {
        PopoConfig {
            enabled: false,
            alias: default_alias(),
            fingerprint: String::new(),
            target: None,
        }
    }
}

/// Window-behaviour preferences (edge auto-hide, minimize-to-tray, desktop widget).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowConfig {
    #[serde(default = "default_true")]
    pub edge_hide: bool,
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub widget: bool,
    #[serde(default)]
    pub widget_edge: Option<WidgetEdge>,
    /// Last user-selected vertical origin in logical desktop coordinates.
    #[serde(default)]
    pub widget_y: Option<f64>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        WindowConfig {
            edge_hide: true,
            minimize_to_tray: true,
            widget: false,
            widget_edge: None,
            widget_y: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WidgetEdge {
    Left,
    Right,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Db {
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub executions: Vec<Execution>,
    #[serde(default)]
    pub results: HashMap<Id, TaskResultState>,
    #[serde(default)]
    pub notifications: Vec<Notification>,
    #[serde(default)]
    pub providers: Vec<AiProvider>,
    #[serde(default)]
    pub active_provider: Option<Id>,
    #[serde(default)]
    pub popo: PopoConfig,
    #[serde(default)]
    pub bot_channels: Vec<BotChannel>,
    #[serde(default)]
    pub community: CommunityConfig,
    /// One-time migration marker: removes legacy demo tasks and installs the real
    /// example tasks once.
    #[serde(default)]
    pub migrated_v1: bool,
    /// Second migration: installs the additional example tasks once.
    #[serde(default)]
    pub migrated_v2: bool,
    #[serde(default)]
    pub window: WindowConfig,
    /// Third migration: installs the first built-in plugin task once.
    #[serde(default)]
    pub migrated_v3: bool,
    /// Fourth migration: separates library activation from the runtime switch.
    #[serde(default)]
    pub migrated_v4: bool,
    /// Fifth migration: old prototype webhook channels were never true local
    /// robot connections. Keep their data readable, but stop and shelve them.
    #[serde(default)]
    pub migrated_v5: bool,
    /// Sixth migration: add the public, same-origin GitHub community registry
    /// without replacing any sources the user already configured.
    #[serde(default)]
    pub migrated_v6: bool,
    /// Seventh migration: remove superseded showcase tasks. Distribution keeps
    /// only the weather example and the disabled CLIProxyAPI plugin entry.
    #[serde(default)]
    pub migrated_v7: bool,
}
