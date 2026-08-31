import { useEffect, useState } from "react";
import { Archive, AlertTriangle, ChevronDown, ChevronRight, Pencil, Play, Plus, Power, Search, ShieldCheck, Trash2, X } from "lucide-react";
import { api, errorMessage } from "../api";
import type { CapabilityProfile, DisplayForm, Lifecycle, Snapshot, Task, Trigger } from "../types";

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

function humanSecs(s: number): string {
  if (s <= 0) return "—";
  if (s % 3600 === 0) return `${s / 3600} 小时`;
  if (s % 60 === 0) return `${s / 60} 分钟`;
  return `${s} 秒`;
}

const WD = ["日", "一", "二", "三", "四", "五", "六"];

function triggerLabel(t: Trigger): string {
  switch (t.kind) {
    case "interval":
      return `每 ${humanSecs(t.everySecs ?? 0)}`;
    case "daily":
      return `每日 ${t.at}`;
    case "weekly":
      return `每周${(t.days ?? []).map((d) => WD[d] ?? "").join("") || "?"} ${t.at}`;
    case "monthly":
      return `每月${t.day}号 ${t.at}`;
    case "watch":
      return "看守";
    default:
      return "手动";
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
    setImportMsg("正在识别入口…");
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
      setImportMsg(`识别为 ${g.note}` + (g.outputDir ? `，产物目录 ${g.outputDir}` : ""));
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
      setFormError("请填写任务昵称。");
      return;
    }
    if (!command) {
      setFormError("请填写要运行的命令。");
      return;
    }
    if (form.trigger.kind === "interval" && (form.trigger.everySecs ?? 0) < 1) {
      setFormError("运行间隔必须至少为 1 秒。");
      return;
    }
    if (form.trigger.kind === "weekly" && !(form.trigger.days ?? []).length) {
      setFormError("每周任务至少选择一天。");
      return;
    }
    if (form.trigger.kind === "watch" && !form.trigger.path?.trim()) {
      setFormError("请填写要看守的目录。");
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
      setScanMsg("请先粘贴要扫描的脚本内容。");
      return;
    }
    setScanMsg("正在扫描…");
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
            <label className="switch" title={t.enabled ? `关闭运行开关${t.kind === "plugin" ? "并终止插件" : ""}` : `打开运行开关${t.kind === "plugin" ? "并启动插件" : ""}`}>
              <input
                aria-label={`${t.enabled ? "关闭" : "打开"}${t.nickname}运行开关`}
                type="checkbox"
                checked={t.enabled}
                onChange={(event) => api.setEnabled(t.id, event.target.checked)}
              />
              <span />
            </label>
          )}
          <span className={"dot " + (running ? "ok" : t.enabled ? "info" : "idle")} />
          <span className="row-name">{t.nickname}</span>
          <span className="tag">{t.kind === "plugin" ? "插件" : shelved ? "脚本" : triggerLabel(t.trigger)}</span>
          {t.community && <span className="tag community-tag">社区 · {t.community.author}</span>}
          {shelved && <span className="tag">未启用</span>}
          {!shelved && t.kind === "plugin" && <span className={"tag" + (running ? " success" : "")}>{running ? "运行中" : t.enabled ? "启动中" : "运行开关已关"}</span>}
          {!shelved && t.kind !== "plugin" && t.lifecycle === "resident" && <span className="tag">保持运行</span>}
          {shelved ? (
            <button className="btn small" onClick={() => api.setActive(t.id, true)} title="启用到工作区">
              <Power size={13} /> 启用
            </button>
          ) : (
            <button className="btn small" onClick={() => api.setActive(t.id, false)} title="移到未启用收纳区">
              <Archive size={13} /> 收纳
            </button>
          )}
          <button
            className="btn small"
            onClick={() => startEdit(t)}
            disabled={Boolean(t.community)}
            aria-label={`编辑${t.nickname}`}
            title={t.community ? "社区包的入口由注册表管理" : "编辑任务"}
          >
            <Pencil size={13} />
          </button>
          {!shelved && t.kind !== "plugin" && (
            <button className="btn small" onClick={() => api.runNow(t.id)} disabled={running || !t.enabled} title={!t.enabled ? "先打开运行开关" : "立即运行"}>
              <Play size={13} /> {running ? "运行中" : "运行"}
            </button>
          )}
          <button
            className="btn small danger"
            disabled={Boolean(t.community)}
            aria-label={`删除${t.nickname}`}
            title={t.community ? "请在插件中心卸载并清理包文件" : "删除任务"}
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
          {t.cwd && <span>工作目录：{t.cwd}</span>}
        </div>
      </div>
    );
  };

  return (
    <div className="view">
      <div className="view-head">
        <h2 className="view-title">脚本与插件</h2>
        <span className="muted" />
        <button className="btn" onClick={startNew}>
          <Plus size={14} /> 添加
        </button>
      </div>

      {!warnHidden && (
        <div className="warn-banner">
          <AlertTriangle size={14} />
          <span style={{ flex: 1 }}>脚本以普通子进程运行，运行时隔离尚未启用——只导入你信任的脚本。</span>
          <button
            className="warn-close"
            aria-label="不再提示"
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
            placeholder="搜索脚本或插件…"
          />
        </div>
      )}

      <div className="rows">
        {plugins.length > 0 && <div className="task-section-head"><span>已启用 · 插件</span><span>{plugins.length}</span></div>}
        {plugins.map((task) => taskRow(task))}
        {scripts.length > 0 && <div className="task-section-head"><span>已启用 · 脚本</span><span>{scripts.length}</span></div>}
        {scripts.map((task) => taskRow(task))}
        {inactive.length > 0 && (
          <div className="inactive-shelf">
            <button
              className="inactive-shelf-head"
              onClick={() => setInactiveOpen((open) => !open)}
              aria-expanded={inactiveOpen || Boolean(q)}
            >
              {inactiveOpen || q ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              <span>未启用 · 已收纳</span>
              <span className="inactive-count">{inactive.length}</span>
              <small>不会调度或运行</small>
            </button>
            {(inactiveOpen || Boolean(q)) && <div className="inactive-shelf-body">{inactive.map((task) => taskRow(task, true))}</div>}
          </div>
        )}
        {snap.tasks.length === 0 && <div className="empty">库里还没有脚本或插件。点右上「添加」开始收纳。</div>}
        {snap.tasks.length > 0 && filtered.length === 0 && <div className="empty">没有匹配的脚本或插件。</div>}
      </div>

      {form && (
        <div className="modal" onClick={() => setForm(null)}>
          <div className="sheet wide" role="dialog" aria-modal="true" aria-labelledby="task-form-title" onClick={(e) => e.stopPropagation()}>
            <div className="sheet-head">
              <h3 id="task-form-title">{form.id ? `编辑${form.kind === "plugin" ? "插件" : "脚本"}` : "添加脚本或插件"}</h3>
              <button className="icon-btn" onClick={() => setForm(null)} aria-label="关闭任务编辑器"><X size={16} /></button>
            </div>

            <div className="sheet-section">基本信息</div>
            <label>
              从本地项目 / 脚本导入（可选）
              <input
                value={importPath}
                onChange={(e) => setImportPath(e.target.value)}
                placeholder="粘贴目录或脚本文件的完整路径，自动识别入口"
              />
            </label>
            <div>
              <button className="btn small" type="button" onClick={doImport} disabled={importBusy || !importPath.trim()}>
                {importBusy ? "识别中…" : "识别入口"}
              </button>
              {importMsg && <span className="muted small" style={{ marginLeft: 8 }}>{importMsg}</span>}
            </div>
            <div className="sheet-grid">
              <label>
                类型
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
                  <option value="script">脚本</option>
                  <option value="plugin">插件 / 常驻服务</option>
                </select>
              </label>
              <label>
                昵称
                <input autoFocus required value={form.nickname} onChange={(e) => setForm({ ...form, nickname: e.target.value })} />
              </label>
            </div>
            <label>
              命令
              <input required value={form.command} onChange={(e) => setForm({ ...form, command: e.target.value })} />
            </label>
            <label>
              参数（每行一个）
              <textarea value={argsText} onChange={(e) => setArgsText(e.target.value)} rows={3} />
            </label>
            <label>
              交互输入（可选，喂给脚本 stdin，按提示顺序每行一个）
              <textarea
                value={form.stdin ?? ""}
                onChange={(e) => setForm({ ...form, stdin: e.target.value || null })}
                rows={2}
                placeholder="如脚本会提示输入网址，这里就填那个网址"
              />
            </label>

            <div className="sheet-section">触发方式</div>
            {form.kind === "plugin" && <div className="plugin-hint">插件由任务列表开关控制：开启即静默启动，关闭即终止进程树。</div>}
            <div className="sheet-grid">
              <label>
                触发
                <select
                  value={form.trigger.kind}
                  disabled={form.kind === "plugin"}
                  onChange={(e) => setForm({ ...form, trigger: triggerForKind(e.target.value) })}
                >
                  <option value="manual">手动</option>
                  <option value="interval">定时（间隔）</option>
                  <option value="daily">每日定点</option>
                  <option value="weekly">每周</option>
                  <option value="monthly">每月</option>
                  <option value="watch">文件看守</option>
                </select>
              </label>
              {form.trigger.kind === "interval" && (
                <label>
                  每隔（秒）
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
                  时间 HH:MM
                  <input
                    type="time"
                    value={form.trigger.at ?? "09:00"}
                    onChange={(e) => setForm({ ...form, trigger: { kind: "daily", at: e.target.value } })}
                  />
                </label>
              )}
              {form.trigger.kind === "monthly" && (
                <label>
                  每月几号
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
                  时间 HH:MM
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
                星期几
                <div className="weekday-row">
                  {WD.map((label, i) => {
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
                  看守目录
                  <input
                    value={form.trigger.path ?? ""}
                    onChange={(e) =>
                      setForm({ ...form, trigger: { ...form.trigger, kind: "watch", path: e.target.value } })
                    }
                  />
                </label>
                <div className="sheet-grid">
                  <label>
                    文件匹配（如 *.pdf, *.png）
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
                    递归子目录
                  </label>
                </div>
              </>
            )}

            <div className="sheet-section">展示与生命周期</div>
            <div className="sheet-grid">
              <label>
                展示形态
                <select value={form.display} onChange={(e) => setForm({ ...form, display: e.target.value as DisplayForm })}>
                  <option value="card">模块卡</option>
                  <option value="strip">推送横条</option>
                  <option value="metric">指标</option>
                </select>
              </label>
              <label>
                生命周期
                <select value={form.lifecycle} onChange={(e) => setForm({ ...form, lifecycle: e.target.value as Lifecycle })}>
                  <option value="ephemeral">处理完关闭</option>
                  <option value="resident">保持运行</option>
                </select>
              </label>
            </div>

            <div className="sheet-section">高级</div>
            <div className="sheet-grid">
              <label>
                超时（秒，0 = 不限）
                <input
                  type="number"
                  value={form.timeoutSecs}
                  onChange={(e) => setForm({ ...form, timeoutSecs: num(e.target.value, 0) })}
                />
              </label>
              <label>
                完成后推送到
                <select
                  value={form.pushChannel ?? ""}
                  onChange={(e) => setForm({ ...form, pushChannel: e.target.value || null })}
                >
                  <option value="">不推送</option>
                  {snap.popo.enabled && snap.popo.target && <option value="popo">PoPo · {snap.popo.target.alias}</option>}
                  {snap.botChannels
                    .filter((channel) => channel.enabled && channel.secretConfigured)
                    .map((channel) => <option key={channel.id} value={`bot:${channel.id}`}>{channel.name}</option>)}
                  {form.pushChannel === "popo" && (!snap.popo.enabled || !snap.popo.target) && <option value="popo">PoPo 已收纳或未配置</option>}
                  {form.pushChannel
                    && form.pushChannel !== "popo"
                    && !snap.botChannels.some((channel) => `bot:${channel.id}` === form.pushChannel && channel.enabled)
                    && <option value={form.pushChannel}>原渠道已收纳或删除</option>}
                </select>
              </label>
            </div>
            <div className="sheet-grid">
              <label>
                工作目录（cwd，可选）
                <input
                  value={form.cwd ?? ""}
                  onChange={(e) => setForm({ ...form, cwd: e.target.value || null })}
                  placeholder="脚本运行时的当前目录"
                />
              </label>
              <label>
                产物目录（可选）
                <input
                  value={form.outputDir ?? ""}
                  onChange={(e) => setForm({ ...form, outputDir: e.target.value || null })}
                  placeholder="展示该目录最新的文件"
                />
              </label>
            </div>

            {formScan && (
              <div className="scan-result risk-high">
                <div className="scan-risk">
                  <AlertTriangle size={14} /> {formScan.risk === "unknown" ? "源码不可判定" : "高风险脚本"}
                </div>
                <div className="scan-summary">{formScan.summary}</div>
                <div className="scan-hosts">检查范围：{formScan.scope}</div>
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
                取消
              </button>
              {formScan ? (
                <button className="btn danger" disabled={saving} onClick={() => void save(true)}>
                  {formScan.risk === "unknown" ? "理解无法判定，仍要保存" : "仍要保存（高风险）"}
                </button>
              ) : (
                <button className="btn primary" disabled={saving} onClick={() => void save()}>
                  {saving ? "保存中…" : "保存"}
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      {deleteTarget && (
        <div className="modal" onClick={() => setDeleteTarget(null)}>
          <div className="sheet" role="alertdialog" aria-modal="true" aria-labelledby="delete-task-title" onClick={(e) => e.stopPropagation()} style={{ width: 380 }}>
            <h3 id="delete-task-title">删除任务</h3>
            <div className="scan-summary">
              确定删除「{deleteTarget.nickname}」？默认会<b>保留</b>已采集的产物记录。
            </div>
            <label className="row-inline">
              <input type="checkbox" checked={deleteProducts} onChange={(e) => setDeleteProducts(e.target.checked)} />
              同时删除已采集的产物
            </label>
            {deleteError && <div className="form-error" role="alert"><AlertTriangle size={14} /> {deleteError}</div>}
            <div className="sheet-actions">
              <button className="btn" onClick={() => setDeleteTarget(null)}>
                取消
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
                {deleting ? "删除中…" : "删除"}
              </button>
            </div>
          </div>
        </div>
      )}

      <div className="scan-tool">
        <h3>
          <ShieldCheck size={16} /> 源码风险提示（不是病毒查杀）
        </h3>
        <textarea
          placeholder="把脚本文本贴进来，查看已知危险能力；保存本地任务时会自动读取实际入口和项目源码…"
          value={scanSrc}
          onChange={(e) => setScanSrc(e.target.value)}
          rows={5}
        />
        <div>
          <button className="btn" onClick={doScan} disabled={!scanSrc.trim()}>
            扫描
          </button>
        </div>
        {scanMsg && <div className="muted small" role="status">{scanMsg}</div>}
        {scan && (
            <div className={"scan-result risk-" + scan.risk}>
            <div className="scan-risk">风险：{scan.risk === "unknown" ? "不可判定" : scan.risk}</div>
            <div className="scan-summary">{scan.summary}</div>
            <div className="scan-hosts">检查范围：{scan.scope}</div>
            {scan.hosts.length > 0 && <div className="scan-hosts">外连主机：{scan.hosts.join(", ")}</div>}
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
