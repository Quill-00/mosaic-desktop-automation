import { useEffect, useState } from "react";
import { Archive, AlertTriangle, ChevronDown, ChevronRight, Pencil, Play, Plus, Power, Search, ShieldCheck, Trash2, X } from "lucide-react";
import { api, errorMessage } from "../api";
import type { CapabilityProfile, DisplayForm, Lifecycle, Snapshot, Task, Trigger } from "../types";
import { useI18n } from "../i18n";
import type { Locale } from "../i18n";

function emptyTask(): Task {
  return {
    id: "",
    nickname: "",
    command: "node",
    kind: "script",
    args: [],
    cwd: null,
    trigger: { kind: "interval", everySecs: 60 },
    display: "card",
    lifecycle: "ephemeral",
    active: true,
    enabled: true,
    timeoutSecs: 0,
    pushChannel: null,
    outputDir: null,
    stdin: null,
    createdAt: "",
  };
}

function num(v: string, fallback: number): number {
  const n = Number(v);
  return Number.isFinite(n) ? n : fallback;
}

function humanSecs(s: number, locale: Locale): string {
  if (s <= 0) return "—";
  if (s % 3600 === 0) return locale === "zh-CN" ? `${s / 3600} 小时` : `${s / 3600} hr`;
  if (s % 60 === 0) return locale === "zh-CN" ? `${s / 60} 分钟` : `${s / 60} min`;
  return locale === "zh-CN" ? `${s} 秒` : `${s} sec`;
}

const WD_ZH = ["日", "一", "二", "三", "四", "五", "六"];
const WD_EN = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

function triggerLabel(trigger: Trigger, locale: Locale): string {
  switch (trigger.kind) {
    case "interval":
      return locale === "zh-CN" ? `每 ${humanSecs(trigger.everySecs ?? 0, locale)}` : `Every ${humanSecs(trigger.everySecs ?? 0, locale)}`;
    case "daily":
      return locale === "zh-CN" ? `每日 ${trigger.at}` : `Daily ${trigger.at}`;
    case "weekly":
      return locale === "zh-CN" ? `每周${(trigger.days ?? []).map((d) => WD_ZH[d] ?? "").join("") || "?"} ${trigger.at}` : `Weekly ${(trigger.days ?? []).map((d) => WD_EN[d] ?? "").join(", ") || "?"} ${trigger.at}`;
    case "monthly":
      return locale === "zh-CN" ? `每月${trigger.day}号 ${trigger.at}` : `Monthly, day ${trigger.day} at ${trigger.at}`;
    case "watch":
      return locale === "zh-CN" ? "看守" : "Watch";
    default:
      return locale === "zh-CN" ? "手动" : "Manual";
  }
}

function triggerForKind(k: string): Trigger {
  switch (k) {
    case "interval":
      return { kind: "interval", everySecs: 60 };
    case "daily":
      return { kind: "daily", at: "09:00" };
    case "weekly":
      return { kind: "weekly", days: [1], at: "09:00" };
    case "monthly":
      return { kind: "monthly", day: 1, at: "09:00" };
    case "watch":
      return { kind: "watch", path: "", pattern: "*", recursive: true };
    default:
      return { kind: "manual" };
  }
}

export default function Tasks({
  snap,
  autoNew,
  onAutoNew,
}: {
  snap: Snapshot;
  autoNew: boolean;
  onAutoNew: () => void;
}) {
  const { locale, t: tr } = useI18n();
  const [form, setForm] = useState<Task | null>(null);
  const [argsText, setArgsText] = useState("");
  const [formScan, setFormScan] = useState<CapabilityProfile | null>(null);
  const [formScanSignature, setFormScanSignature] = useState("");
  const [formError, setFormError] = useState("");
  const [saving, setSaving] = useState(false);
  const [scanSrc, setScanSrc] = useState("");
  const [scan, setScan] = useState<CapabilityProfile | null>(null);
  const [scanMsg, setScanMsg] = useState("");
  const [importPath, setImportPath] = useState("");
  const [importMsg, setImportMsg] = useState("");
  const [importBusy, setImportBusy] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Task | null>(null);
  const [deleteProducts, setDeleteProducts] = useState(false);
  const [deleteError, setDeleteError] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [warnHidden, setWarnHidden] = useState(() => localStorage.getItem("mosaic.warnDismissed") === "1");
  const [query, setQuery] = useState("");
  const [inactiveOpen, setInactiveOpen] = useState(false);

  function startNew() {
    setForm(emptyTask());
    setArgsText("");
    setFormScan(null);
    setFormScanSignature("");
    setFormError("");
    setImportPath("");
    setImportMsg("");
  }

  // Opens the new-task form whenever the sidebar "新建任务" button flips autoNew —
  // works whether this view was just mounted or is already showing.
  useEffect(() => {
    if (autoNew) {
      startNew();
      onAutoNew();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoNew]);

  function startEdit(t: Task) {
    setForm(t);
    setArgsText(t.args.join("\n"));
    setFormScan(null);
    setFormScanSignature("");
    setFormError("");
    setImportPath("");
    setImportMsg("");
  }

  async function doImport() {
    if (!form || !importPath.trim() || importBusy) return;
    setImportBusy(true);
    setImportMsg(tr("Detecting entry point…", "正在识别入口…"));
    try {
      const g = await api.importLocal(importPath.trim());
      setForm({
        ...form,
        command: g.command,
        args: g.args,
        cwd: g.cwd,
        nickname: form.nickname || g.nickname,
        outputDir: g.outputDir || form.outputDir,
      });
      setArgsText(g.args.join("\n"));
      setImportMsg(tr(`Detected: ${g.note}${g.outputDir ? `; output folder ${g.outputDir}` : ""}`, `识别为 ${g.note}${g.outputDir ? `，产物目录 ${g.outputDir}` : ""}`));
    } catch (e) {
      setImportMsg(errorMessage(e));
    } finally {
      setImportBusy(false);
    }
  }

  async function save(forceHighRisk = false) {
    if (!form || saving) return;
    const args = argsText.split("\n").map((s) => s.trim()).filter(Boolean);
    const nickname = form.nickname.trim();
    const command = form.command.trim();
    if (!nickname) {
      setFormError(tr("Enter a task name.", "请填写任务昵称。"));
      return;
    }
    if (!command) {
      setFormError(tr("Enter a command to run.", "请填写要运行的命令。"));
      return;
    }
    if (form.trigger.kind === "interval" && (form.trigger.everySecs ?? 0) < 1) {
      setFormError(tr("The interval must be at least one second.", "运行间隔必须至少为 1 秒。"));
      return;
    }
    if (form.trigger.kind === "weekly" && !(form.trigger.days ?? []).length) {
      setFormError(tr("Select at least one weekday.", "每周任务至少选择一天。"));
      return;
    }
    if (form.trigger.kind === "watch" && !form.trigger.path?.trim()) {
      setFormError(tr("Enter a folder to watch.", "请填写要看守的目录。"));
      return;
    }

    setSaving(true);
    setFormError("");
    try {
      const profile = await api.scanTaskSource(command, args, form.cwd);
      const scanSignature = JSON.stringify({ command, args, cwd: form.cwd ?? "" });
      if (
        (profile.risk === "high" || profile.risk === "unknown")
        && (!forceHighRisk || formScanSignature !== scanSignature)
      ) {
        setFormScan(profile);
        setFormScanSignature(scanSignature);
        return;
      }
      await api.createTask({ ...form, nickname, command, args });
      setForm(null);
      setFormScan(null);
    } catch (e) {
      setFormError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  }

  async function doScan() {
    if (!scanSrc.trim()) {
      setScanMsg(tr("Paste script source before scanning.", "请先粘贴要扫描的脚本内容。"));
      return;
    }
    setScanMsg(tr("Scanning…", "正在扫描…"));
    try {
      setScan(await api.scanScript(scanSrc));
      setScanMsg("");
    } catch (e) {
      setScanMsg(errorMessage(e));
    }
  }

  useEffect(() => {
    if (!form && !deleteTarget) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (deleteTarget) setDeleteTarget(null);
      else setForm(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [form, deleteTarget]);

  const q = query.trim().toLowerCase();
  const filtered = q
    ? snap.tasks.filter((t) => t.nickname.toLowerCase().includes(q) || t.command.toLowerCase().includes(q))
    : snap.tasks;
  const plugins = filtered.filter((task) => task.active && task.kind === "plugin");
  const scripts = filtered.filter((task) => task.active && task.kind !== "plugin");
  const inactive = filtered.filter((task) => !task.active);

  const taskRow = (t: Task, shelved = false) => {
    const running = snap.running.some((process) => process.taskId === t.id);
    return (
      <div key={t.id} className={"row task-row" + (running ? " is-running" : "") + (shelved ? " is-shelved" : "")}>
        <div className="row-main">
          {!shelved && (
            <label className="switch" title={t.enabled ? tr(t.kind === "plugin" ? "Turn off and stop the plugin" : "Turn off execution", `关闭运行开关${t.kind === "plugin" ? "并终止插件" : ""}`) : tr(t.kind === "plugin" ? "Turn on and start the plugin" : "Turn on execution", `打开运行开关${t.kind === "plugin" ? "并启动插件" : ""}`)}>
              <input
                aria-label={tr(`${t.enabled ? "Turn off" : "Turn on"} ${t.nickname}`, `${t.enabled ? "关闭" : "打开"}${t.nickname}运行开关`)}
                type="checkbox"
                checked={t.enabled}
                onChange={(event) => api.setEnabled(t.id, event.target.checked)}
              />
              <span />
            </label>
          )}
          <span className={"dot " + (running ? "ok" : t.enabled ? "info" : "idle")} />
          <span className="row-name">{t.nickname}</span>
          <span className="tag">{t.kind === "plugin" ? tr("Plugin", "插件") : shelved ? tr("Script", "脚本") : triggerLabel(t.trigger, locale)}</span>
          {t.community && <span className="tag community-tag">{tr("Community", "社区")} · {t.community.author}</span>}
          {shelved && <span className="tag">{tr("Disabled", "未启用")}</span>}
          {!shelved && t.kind === "plugin" && <span className={"tag" + (running ? " success" : "")}>{running ? tr("Running", "运行中") : t.enabled ? tr("Starting", "启动中") : tr("Execution off", "运行开关已关")}</span>}
          {!shelved && t.kind !== "plugin" && t.lifecycle === "resident" && <span className="tag">{tr("Keep running", "保持运行")}</span>}
          {shelved ? (
            <button className="btn small" onClick={() => api.setActive(t.id, true)} title={tr("Enable in workspace", "启用到工作区")}>
              <Power size={13} /> {tr("Enable", "启用")}
            </button>
          ) : (
            <button className="btn small" onClick={() => api.setActive(t.id, false)} title={tr("Move to the disabled shelf", "移到未启用收纳区")}>
              <Archive size={13} /> {tr("Shelve", "收纳")}
            </button>
          )}
          <button
            className="btn small"
            onClick={() => startEdit(t)}
            disabled={Boolean(t.community)}
            aria-label={tr(`Edit ${t.nickname}`, `编辑${t.nickname}`)}
            title={t.community ? tr("Community package entry points are managed by the registry", "社区包的入口由注册表管理") : tr("Edit task", "编辑任务")}
          >
            <Pencil size={13} />
          </button>
          {!shelved && t.kind !== "plugin" && (
            <button className="btn small" onClick={() => api.runNow(t.id)} disabled={running || !t.enabled} title={!t.enabled ? tr("Turn on execution first", "先打开运行开关") : tr("Run now", "立即运行")}>
              <Play size={13} /> {running ? tr("Running", "运行中") : tr("Run", "运行")}
            </button>
          )}
          <button
            className="btn small danger"
            disabled={Boolean(t.community)}
            aria-label={tr(`Delete ${t.nickname}`, `删除${t.nickname}`)}
            title={t.community ? tr("Uninstall community packages from the Community view", "请在插件中心卸载并清理包文件") : tr("Delete task", "删除任务")}
            onClick={() => {
              setDeleteTarget(t);
              setDeleteProducts(false);
              setDeleteError("");
            }}
          >
            <Trash2 size={13} />
          </button>
        </div>
        <div className="row-meta task-command">
          <span className="mono cmd">{t.command}{t.args.length ? ` ${t.args.join(" ")}` : ""}</span>
          {t.cwd && <span>{tr("Working directory", "工作目录")}: {t.cwd}</span>}
        </div>
      </div>
    );
  };

  return (
    <div className="view">
      <div className="view-head">
        <h2 className="view-title">{tr("Scripts & plugins", "脚本与插件")}</h2>
        <span className="muted" />
        <button className="btn" onClick={startNew}>
          <Plus size={14} /> {tr("Add", "添加")}
        </button>
      </div>

      {!warnHidden && (
        <div className="warn-banner">
          <AlertTriangle size={14} />
          <span style={{ flex: 1 }}>{tr("Scripts run as regular child processes and are not sandboxed. Only import code you trust.", "脚本以普通子进程运行，运行时隔离尚未启用——只导入你信任的脚本。")}</span>
          <button
            className="warn-close"
            aria-label={tr("Dismiss permanently", "不再提示")}
            onClick={() => {
              localStorage.setItem("mosaic.warnDismissed", "1");
              setWarnHidden(true);
            }}
          >
            <X size={14} />
          </button>
        </div>
      )}

      {snap.tasks.length > 0 && (
        <div className="search-box">
          <Search size={15} className="muted" />
          <input
            className="nl-input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={tr("Search scripts or plugins…", "搜索脚本或插件…")}
          />
        </div>
      )}

      <div className="rows">
        {plugins.length > 0 && <div className="task-section-head"><span>{tr("Enabled · Plugins", "已启用 · 插件")}</span><span>{plugins.length}</span></div>}
        {plugins.map((task) => taskRow(task))}
        {scripts.length > 0 && <div className="task-section-head"><span>{tr("Enabled · Scripts", "已启用 · 脚本")}</span><span>{scripts.length}</span></div>}
        {scripts.map((task) => taskRow(task))}
        {inactive.length > 0 && (
          <div className="inactive-shelf">
            <button
              className="inactive-shelf-head"
              onClick={() => setInactiveOpen((open) => !open)}
              aria-expanded={inactiveOpen || Boolean(q)}
            >
              {inactiveOpen || q ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              <span>{tr("Disabled · Shelved", "未启用 · 已收纳")}</span>
              <span className="inactive-count">{inactive.length}</span>
              <small>{tr("Will not be scheduled or run", "不会调度或运行")}</small>
            </button>
            {(inactiveOpen || Boolean(q)) && <div className="inactive-shelf-body">{inactive.map((task) => taskRow(task, true))}</div>}
          </div>
        )}
        {snap.tasks.length === 0 && <div className="empty">{tr("Your library is empty. Select Add to import a script or plugin.", "库里还没有脚本或插件。点右上「添加」开始收纳。")}</div>}
        {snap.tasks.length > 0 && filtered.length === 0 && <div className="empty">{tr("No matching scripts or plugins.", "没有匹配的脚本或插件。")}</div>}
      </div>

      {form && (
        <div className="modal" onClick={() => setForm(null)}>
          <div className="sheet wide" role="dialog" aria-modal="true" aria-labelledby="task-form-title" onClick={(e) => e.stopPropagation()}>
            <div className="sheet-head">
              <h3 id="task-form-title">{form.id ? tr(`Edit ${form.kind === "plugin" ? "plugin" : "script"}`, `编辑${form.kind === "plugin" ? "插件" : "脚本"}`) : tr("Add script or plugin", "添加脚本或插件")}</h3>
              <button className="icon-btn" onClick={() => setForm(null)} aria-label={tr("Close task editor", "关闭任务编辑器")}><X size={16} /></button>
            </div>

            <div className="sheet-section">{tr("Basics", "基本信息")}</div>
            <label>
              {tr("Import from a local project or script (optional)", "从本地项目 / 脚本导入（可选）")}
              <input
                value={importPath}
                onChange={(e) => setImportPath(e.target.value)}
                placeholder={tr("Paste the full path to a folder or script to detect its entry point", "粘贴目录或脚本文件的完整路径，自动识别入口")}
              />
            </label>
            <div>
              <button className="btn small" type="button" onClick={doImport} disabled={importBusy || !importPath.trim()}>
                {importBusy ? tr("Detecting…", "识别中…") : tr("Detect entry point", "识别入口")}
              </button>
              {importMsg && <span className="muted small" style={{ marginLeft: 8 }}>{importMsg}</span>}
            </div>
            <div className="sheet-grid">
              <label>
                {tr("Type", "类型")}
                <select
                  value={form.kind}
                  onChange={(event) => {
                    const kind = event.target.value as Task["kind"];
                    setForm({
                      ...form,
                      kind,
                      ...(kind === "plugin"
                        ? { lifecycle: "resident" as const, trigger: { kind: "manual" as const }, timeoutSecs: 0, onDashboard: false }
                        : {}),
                    });
                  }}
                >
                  <option value="script">{tr("Script", "脚本")}</option>
                  <option value="plugin">{tr("Plugin / resident service", "插件 / 常驻服务")}</option>
                </select>
              </label>
              <label>
                {tr("Name", "昵称")}
                <input autoFocus required value={form.nickname} onChange={(e) => setForm({ ...form, nickname: e.target.value })} />
              </label>
            </div>
            <label>
              {tr("Command", "命令")}
              <input required value={form.command} onChange={(e) => setForm({ ...form, command: e.target.value })} />
            </label>
            <label>
              {tr("Arguments (one per line)", "参数（每行一个）")}
              <textarea value={argsText} onChange={(e) => setArgsText(e.target.value)} rows={3} />
            </label>
            <label>
              {tr("Interactive input (optional; one stdin response per line)", "交互输入（可选，喂给脚本 stdin，按提示顺序每行一个）")}
              <textarea
                value={form.stdin ?? ""}
                onChange={(e) => setForm({ ...form, stdin: e.target.value || null })}
                rows={2}
                placeholder={tr("For example, enter the URL requested by the script", "如脚本会提示输入网址，这里就填那个网址")}
              />
            </label>

            <div className="sheet-section">{tr("Trigger", "触发方式")}</div>
            {form.kind === "plugin" && <div className="plugin-hint">{tr("Use the task-list switch to control plugins: on starts silently; off terminates the process tree.", "插件由任务列表开关控制：开启即静默启动，关闭即终止进程树。")}</div>}
            <div className="sheet-grid">
              <label>
                {tr("Trigger", "触发")}
                <select
                  value={form.trigger.kind}
                  disabled={form.kind === "plugin"}
                  onChange={(e) => setForm({ ...form, trigger: triggerForKind(e.target.value) })}
                >
                  <option value="manual">{tr("Manual", "手动")}</option>
                  <option value="interval">{tr("Interval", "定时（间隔）")}</option>
                  <option value="daily">{tr("Daily", "每日定点")}</option>
                  <option value="weekly">{tr("Weekly", "每周")}</option>
                  <option value="monthly">{tr("Monthly", "每月")}</option>
                  <option value="watch">{tr("File watch", "文件看守")}</option>
                </select>
              </label>
              {form.trigger.kind === "interval" && (
                <label>
                  {tr("Every (seconds)", "每隔（秒）")}
                  <input
                    type="number"
                    min={1}
                    value={form.trigger.everySecs ?? 60}
                    onChange={(e) =>
                      setForm({ ...form, trigger: { kind: "interval", everySecs: num(e.target.value, 60) } })
                    }
                  />
                </label>
              )}
              {form.trigger.kind === "daily" && (
                <label>
                  {tr("Time HH:MM", "时间 HH:MM")}
                  <input
                    type="time"
                    value={form.trigger.at ?? "09:00"}
                    onChange={(e) => setForm({ ...form, trigger: { kind: "daily", at: e.target.value } })}
                  />
                </label>
              )}
              {form.trigger.kind === "monthly" && (
                <label>
                  {tr("Day of month", "每月几号")}
                  <input
                    type="number"
                    min={1}
                    max={31}
                    value={form.trigger.day ?? 1}
                    onChange={(e) =>
                      setForm({ ...form, trigger: { ...form.trigger, kind: "monthly", day: num(e.target.value, 1) } })
                    }
                  />
                </label>
              )}
              {(form.trigger.kind === "weekly" || form.trigger.kind === "monthly") && (
                <label>
                  {tr("Time HH:MM", "时间 HH:MM")}
                  <input
                    type="time"
                    value={form.trigger.at ?? "09:00"}
                    onChange={(e) =>
                      setForm({ ...form, trigger: { ...form.trigger, at: e.target.value } })
                    }
                  />
                </label>
              )}
            </div>
            {form.trigger.kind === "weekly" && (
              <label>
                {tr("Weekdays", "星期几")}
                <div className="weekday-row">
                  {(locale === "zh-CN" ? WD_ZH : WD_EN).map((label, i) => {
                    const on = (form.trigger.days ?? []).includes(i);
                    return (
                      <button
                        key={i}
                        type="button"
                        className={"wd-btn" + (on ? " on" : "")}
                        onClick={() => {
                          const cur = new Set(form.trigger.days ?? []);
                          if (cur.has(i)) cur.delete(i);
                          else cur.add(i);
                          setForm({
                            ...form,
                            trigger: { ...form.trigger, kind: "weekly", days: Array.from(cur).sort((a, b) => a - b) },
                          });
                        }}
                      >
                        {label}
                      </button>
                    );
                  })}
                </div>
              </label>
            )}
            {form.trigger.kind === "watch" && (
              <>
                <label>
                  {tr("Folder to watch", "看守目录")}
                  <input
                    value={form.trigger.path ?? ""}
                    onChange={(e) =>
                      setForm({ ...form, trigger: { ...form.trigger, kind: "watch", path: e.target.value } })
                    }
                  />
                </label>
                <div className="sheet-grid">
                  <label>
                    {tr("File pattern (for example *.pdf, *.png)", "文件匹配（如 *.pdf, *.png）")}
                    <input
                      value={form.trigger.pattern ?? "*"}
                      onChange={(e) =>
                        setForm({ ...form, trigger: { ...form.trigger, kind: "watch", pattern: e.target.value } })
                      }
                    />
                  </label>
                  <label className="row-inline">
                    <input
                      type="checkbox"
                      checked={form.trigger.recursive ?? true}
                      onChange={(e) =>
                        setForm({ ...form, trigger: { ...form.trigger, kind: "watch", recursive: e.target.checked } })
                      }
                    />
                    {tr("Include subfolders", "递归子目录")}
                  </label>
                </div>
              </>
            )}

            <div className="sheet-section">{tr("Display & lifecycle", "展示与生命周期")}</div>
            <div className="sheet-grid">
              <label>
                {tr("Display", "展示形态")}
                <select value={form.display} onChange={(e) => setForm({ ...form, display: e.target.value as DisplayForm })}>
                  <option value="card">{tr("Module card", "模块卡")}</option>
                  <option value="strip">{tr("Notification strip", "推送横条")}</option>
                  <option value="metric">{tr("Metrics", "指标")}</option>
                </select>
              </label>
              <label>
                {tr("Lifecycle", "生命周期")}
                <select value={form.lifecycle} onChange={(e) => setForm({ ...form, lifecycle: e.target.value as Lifecycle })}>
                  <option value="ephemeral">{tr("Exit when complete", "处理完关闭")}</option>
                  <option value="resident">{tr("Keep running", "保持运行")}</option>
                </select>
              </label>
            </div>

            <div className="sheet-section">{tr("Advanced", "高级")}</div>
            <div className="sheet-grid">
              <label>
                {tr("Timeout (seconds; 0 = unlimited)", "超时（秒，0 = 不限）")}
                <input
                  type="number"
                  value={form.timeoutSecs}
                  onChange={(e) => setForm({ ...form, timeoutSecs: num(e.target.value, 0) })}
                />
              </label>
              <label>
                {tr("Send result to", "完成后推送到")}
                <select
                  value={form.pushChannel ?? ""}
                  onChange={(e) => setForm({ ...form, pushChannel: e.target.value || null })}
                >
                  <option value="">{tr("Do not send", "不推送")}</option>
                  {snap.popo.enabled && snap.popo.target && <option value="popo">PoPo · {snap.popo.target.alias}</option>}
                  {snap.botChannels
                    .filter((channel) => channel.enabled && channel.secretConfigured)
                    .map((channel) => <option key={channel.id} value={`bot:${channel.id}`}>{channel.name}</option>)}
                  {form.pushChannel === "popo" && (!snap.popo.enabled || !snap.popo.target) && <option value="popo">{tr("PoPo is shelved or not configured", "PoPo 已收纳或未配置")}</option>}
                  {form.pushChannel
                    && form.pushChannel !== "popo"
                    && !snap.botChannels.some((channel) => `bot:${channel.id}` === form.pushChannel && channel.enabled)
                    && <option value={form.pushChannel}>{tr("The original channel was shelved or deleted", "原渠道已收纳或删除")}</option>}
                </select>
              </label>
            </div>
            <div className="sheet-grid">
              <label>
                {tr("Working directory (cwd, optional)", "工作目录（cwd，可选）")}
                <input
                  value={form.cwd ?? ""}
                  onChange={(e) => setForm({ ...form, cwd: e.target.value || null })}
                  placeholder={tr("Current directory while the script runs", "脚本运行时的当前目录")}
                />
              </label>
              <label>
                {tr("Output folder (optional)", "产物目录（可选）")}
                <input
                  value={form.outputDir ?? ""}
                  onChange={(e) => setForm({ ...form, outputDir: e.target.value || null })}
                  placeholder={tr("Display the latest files from this folder", "展示该目录最新的文件")}
                />
              </label>
            </div>

            {formScan && (
              <div className="scan-result risk-high">
                <div className="scan-risk">
                  <AlertTriangle size={14} /> {formScan.risk === "unknown" ? tr("Source risk is unknown", "源码不可判定") : tr("High-risk script", "高风险脚本")}
                </div>
                <div className="scan-summary">{formScan.summary}</div>
                <div className="scan-hosts">{tr("Scope", "检查范围")}: {formScan.scope}</div>
                {formScan.findings.length > 0 && (
                  <ul>
                    {formScan.findings.slice(0, 6).map((f, i) => (
                      <li key={i}>
                        <b>{f.capability}</b> · {f.detail}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}

            {formError && <div className="form-error" role="alert"><AlertTriangle size={14} /> {formError}</div>}

            <div className="sheet-actions sticky-actions">
              <button className="btn" onClick={() => setForm(null)}>
                {tr("Cancel", "取消")}
              </button>
              {formScan ? (
                <button className="btn danger" disabled={saving} onClick={() => void save(true)}>
                  {formScan.risk === "unknown" ? tr("Save despite unknown risk", "理解无法判定，仍要保存") : tr("Save high-risk script", "仍要保存（高风险）")}
                </button>
              ) : (
                <button className="btn primary" disabled={saving} onClick={() => void save()}>
                  {saving ? tr("Saving…", "保存中…") : tr("Save", "保存")}
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      {deleteTarget && (
        <div className="modal" onClick={() => setDeleteTarget(null)}>
          <div className="sheet" role="alertdialog" aria-modal="true" aria-labelledby="delete-task-title" onClick={(e) => e.stopPropagation()} style={{ width: 380 }}>
            <h3 id="delete-task-title">{tr("Delete task", "删除任务")}</h3>
            <div className="scan-summary">
              {tr(`Delete “${deleteTarget.nickname}”? Collected output records are `, `确定删除「${deleteTarget.nickname}」？默认会`)}<b>{tr("preserved", "保留")}</b>{tr(" by default.", "已采集的产物记录。")}
            </div>
            <label className="row-inline">
              <input type="checkbox" checked={deleteProducts} onChange={(e) => setDeleteProducts(e.target.checked)} />
              {tr("Also delete collected outputs", "同时删除已采集的产物")}
            </label>
            {deleteError && <div className="form-error" role="alert"><AlertTriangle size={14} /> {deleteError}</div>}
            <div className="sheet-actions">
              <button className="btn" onClick={() => setDeleteTarget(null)}>
                {tr("Cancel", "取消")}
              </button>
              <button
                className="btn danger"
                disabled={deleting}
                onClick={async () => {
                  setDeleting(true);
                  setDeleteError("");
                  try {
                    await api.deleteTask(deleteTarget.id, deleteProducts);
                    setDeleteTarget(null);
                    setDeleteProducts(false);
                  } catch (e) {
                    setDeleteError(errorMessage(e));
                  } finally {
                    setDeleting(false);
                  }
                }}
              >
                {deleting ? tr("Deleting…", "删除中…") : tr("Delete", "删除")}
              </button>
            </div>
          </div>
        </div>
      )}

      <div className="scan-tool">
        <h3>
          <ShieldCheck size={16} /> {tr("Source risk hints (not antivirus scanning)", "源码风险提示（不是病毒查杀）")}
        </h3>
        <textarea
          placeholder={tr("Paste script source to inspect known risky capabilities. Saved local tasks are scanned from their actual entry points and project source.", "把脚本文本贴进来，查看已知危险能力；保存本地任务时会自动读取实际入口和项目源码…")}
          value={scanSrc}
          onChange={(e) => setScanSrc(e.target.value)}
          rows={5}
        />
        <div>
          <button className="btn" onClick={doScan} disabled={!scanSrc.trim()}>
            {tr("Scan", "扫描")}
          </button>
        </div>
        {scanMsg && <div className="muted small" role="status">{scanMsg}</div>}
        {scan && (
            <div className={"scan-result risk-" + scan.risk}>
            <div className="scan-risk">{tr("Risk", "风险")}: {scan.risk === "unknown" ? tr("Unknown", "不可判定") : scan.risk}</div>
            <div className="scan-summary">{scan.summary}</div>
            <div className="scan-hosts">{tr("Scope", "检查范围")}: {scan.scope}</div>
            {scan.hosts.length > 0 && <div className="scan-hosts">{tr("External hosts", "外连主机")}: {scan.hosts.join(", ")}</div>}
            {scan.findings.length > 0 && (
              <ul>
                {scan.findings.map((f, i) => (
                  <li key={i}>
                    <b>{f.capability}</b> · {f.detail}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
