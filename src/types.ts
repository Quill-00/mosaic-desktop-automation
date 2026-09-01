export type Id = string;

export interface Trigger {
  kind: "manual" | "interval" | "daily" | "weekly" | "monthly" | "watch";
  everySecs?: number;
  at?: string;
  days?: number[];
  day?: number;
  path?: string;
  pattern?: string;
  recursive?: boolean;
}

export type DisplayForm = "strip" | "card" | "metric";
export type Lifecycle = "ephemeral" | "resident";

export interface Permissions {
  readPaths: string[];
  writePaths: string[];
  allowHosts: string[];
  channels: string[];
}

export interface Task {
  id: Id;
  nickname: string;
  command: string;
  kind: "script" | "plugin";
  args: string[];
  cwd?: string | null;
  trigger: Trigger;
  display: DisplayForm;
  lifecycle: Lifecycle;
  active: boolean;
  enabled: boolean;
  timeoutSecs: number;
  pushChannel?: string | null;
  onDashboard?: boolean;
  order?: number;
  colSpan?: number;
  rowSpan?: number;
  outputDir?: string | null;
  stdin?: string | null;
  community?: CommunityTaskSource | null;
  createdAt: string;
}

export type PackageRuntime = "node" | "python" | "powerShell" | "executable";

export interface CommunityTaskSource {
  packageId: string;
  version: string;
  registryUrl: string;
  packageDir: string;
  sha256: string;
  author: string;
  runtime: PackageRuntime;
  risk: string;
}

export interface RegistryPackage {
  id: string;
  name: string;
  version: string;
  summary: string;
  description: string;
  author: string;
  license: string;
  repository: string;
  homepage: string;
  tags: string[];
  kind: "script" | "plugin";
  runtime: PackageRuntime;
  entry: string;
  args: string[];
  lifecycle: Lifecycle;
  permissions: Permissions;
  packageUrl: string;
  sha256: string;
}

export interface CommunityCatalogItem {
  package: RegistryPackage;
  sourceUrl: string;
  installedVersion?: string | null;
  installedTaskId?: string | null;
}

export interface CommunityCatalog {
  items: CommunityCatalogItem[];
  errors: { sourceUrl: string; message: string }[];
}

export interface CommunityConfig {
  sources: string[];
}

export interface EntryGuess {
  kind: string;
  command: string;
  args: string[];
  cwd: string;
  nickname: string;
  note: string;
  outputDir: string;
}

export interface PopoPeer {
  ip: string;
  port: number;
  alias: string;
  fingerprint: string;
}

export interface PopoConfig {
  enabled: boolean;
  alias: string;
  fingerprint: string;
  target?: PopoPeer | null;
}

export type BotPlatform = "qq" | "telegram" | "discord" | "slack" | "feishu" | "dingTalk" | "weCom";
export type BotTargetKind = "group" | "c2c";
export type BotConnectionStatus = "stopped" | "connecting" | "online" | "error";

export interface BotChannel {
  id: string;
  name: string;
  platform: BotPlatform;
  appId: string;
  enabled: boolean;
  targetKind: BotTargetKind;
  target: string;
  createdAt: string;
  secretConfigured: boolean;
  status: BotConnectionStatus;
  statusDetail: string;
}

export interface WindowConfig {
  autoStart: boolean;
  edgeHide: boolean;
  minimizeToTray: boolean;
  widget: boolean;
  widgetEdge?: "left" | "right" | null;
  widgetY?: number | null;
}

export interface UpdateStatus {
  currentVersion: string;
  state: "idle" | "checking" | "upToDate" | "ready" | "error";
  latestVersion?: string | null;
  message: string;
  checkedAt?: string | null;
}

export interface Summary {
  headline: string;
  count?: number | null;
  note?: string | null;
}

export interface DetailItem {
  id?: string | null;
  title: string;
  subtitle?: string | null;
  at?: string | null;
  kind?: string | null;
  path?: string | null;
}

export interface TaskResultState {
  summary?: Summary | null;
  card?: any;
  timeline: DetailItem[];
  updatedAt?: string | null;
}

export type ExecStatus = "running" | "ok" | "failed" | "timedOut" | "killed";

export interface Execution {
  id: Id;
  taskId: Id;
  nickname: string;
  startedAt: string;
  finishedAt?: string | null;
  status: ExecStatus;
  exitCode?: number | null;
  trigger: string;
  error?: string | null;
  itemCount: number;
  /// Per-run products; only populated by the on-demand `exec_items` call, not the snapshot.
  items?: DetailItem[];
}

export interface Notification {
  id: Id;
  level: string;
  title: string;
  body?: string | null;
  at: string;
  read: boolean;
  taskId?: string | null;
}

export interface RunningInfo {
  execId: Id;
  taskId: Id;
  nickname: string;
  pid: number;
  startedAt: string;
  lifecycle: Lifecycle;
  command: string;
  uptimeSecs: number;
}

export type ProviderKind = "codex" | "openaiCompatible";
export type AuthMode = "apiKey" | "deviceCode";

export interface AiProvider {
  id: string;
  displayName: string;
  kind: ProviderKind;
  baseUrl: string;
  model: string;
  authMode: AuthMode;
  deviceAuthUrl: string;
  tokenUrl: string;
  clientId: string;
  scope: string;
}

export interface DeviceCodeInfo {
  verificationUri: string;
  userCode: string;
  deviceCode: string;
  interval: number;
  expiresIn: number;
}

export interface PollOut {
  status: string;
  error?: string | null;
}

export interface Snapshot {
  tasks: Task[];
  results: Record<Id, TaskResultState>;
  executions: Execution[];
  notifications: Notification[];
  running: RunningInfo[];
  brief?: Brief;
  popo: PopoConfig;
  botChannels: BotChannel[];
  window: WindowConfig;
  community: CommunityConfig;
  update: UpdateStatus;
}

export interface ChannelInfo {
  id: string;
  name: string;
  configured: boolean;
  note: string;
}

export interface Finding {
  capability: string;
  detail: string;
  severity: string;
}

export interface CapabilityProfile {
  readsFiles: boolean;
  writesFiles: boolean;
  deletesFiles: boolean;
  network: boolean;
  execCommand: boolean;
  readsCredentials: boolean;
  dynamicExec: boolean;
  autostart: boolean;
  hosts: string[];
  findings: Finding[];
  risk: string;
  summary: string;
  scope: string;
  filesScanned: number;
  truncated: boolean;
}

export interface BriefSection {
  icon: string;
  title: string;
  detail: string;
  level: string;
}

export interface Brief {
  headline: string;
  sections: BriefSection[];
}
