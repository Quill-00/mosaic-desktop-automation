import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  Link2,
  PackageCheck,
  Plus,
  RefreshCw,
  Search,
  ShieldAlert,
  Trash2,
  X,
} from "lucide-react";
import { api, errorMessage } from "../api";
import type { CommunityCatalog, CommunityCatalogItem, PackageRuntime, Snapshot } from "../types";
import { useI18n } from "../i18n";
import type { Locale } from "../i18n";

interface RemoveTarget {
  packageId: string;
  name: string;
}

function runtimeName(runtime: PackageRuntime, locale: Locale): string {
  if (runtime === "powerShell") return "PowerShell";
  if (runtime === "executable") return locale === "zh-CN" ? "原生程序" : "Native executable";
  return runtime === "node" ? "Node.js" : "Python";
}

function permissionLabels(item: CommunityCatalogItem, locale: Locale): string[] {
  const permissions = item.package.permissions;
  const labels: string[] = [];
  if (permissions.readPaths.length) labels.push(locale === "zh-CN" ? `读取 ${permissions.readPaths.length} 处路径` : `Read ${permissions.readPaths.length} paths`);
  if (permissions.writePaths.length) labels.push(locale === "zh-CN" ? `写入 ${permissions.writePaths.length} 处路径` : `Write ${permissions.writePaths.length} paths`);
  if (permissions.allowHosts.length) labels.push(locale === "zh-CN" ? `联网 ${permissions.allowHosts.length} 个主机` : `Connect to ${permissions.allowHosts.length} hosts`);
  if (permissions.channels.length) labels.push(locale === "zh-CN" ? `使用 ${permissions.channels.length} 个通道` : `Use ${permissions.channels.length} channels`);
  return labels.length ? labels : [locale === "zh-CN" ? "未声明额外权限" : "No additional permissions declared"];
}

export default function CommunityCenter({ snap }: { snap: Snapshot }) {
  const { locale, t } = useI18n();
  const [sources, setSources] = useState(snap.community.sources);
  const [sourceInput, setSourceInput] = useState("");
  const [catalog, setCatalog] = useState<CommunityCatalog>({ items: [], errors: [] });
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("");
  const [query, setQuery] = useState("");
  const [installTarget, setInstallTarget] = useState<CommunityCatalogItem | null>(null);
  const [removeTarget, setRemoveTarget] = useState<RemoveTarget | null>(null);
  const [busy, setBusy] = useState("");

  useEffect(() => setSources(snap.community.sources), [snap.community.sources]);

  async function refresh() {
    if (!sources.length || loading) {
      if (!sources.length) setCatalog({ items: [], errors: [] });
      return;
    }
    setLoading(true);
    setMessage("");
    try {
      setCatalog(await api.communityCatalog());
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (sources.length) void refresh();
    // Source changes are explicit user actions; avoid refetching on every snapshot poll.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function persistSources(next: string[]) {
    setBusy("sources");
    setMessage("");
    try {
      await api.saveCommunitySources(next);
      setSources(next);
      setSourceInput("");
      setCatalog(next.length ? await api.communityCatalog() : { items: [], errors: [] });
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy("");
    }
  }

  async function install(item: CommunityCatalogItem) {
    setBusy(`install:${item.package.id}`);
    setMessage("");
    try {
      await api.installCommunityPackage(item.sourceUrl, item.package.id, item.package.version);
      setMessage(t(`“${item.package.name}” was installed in the disabled shelf.`, `「${item.package.name}」已安装到未启用收纳区。`));
      setInstallTarget(null);
      setCatalog(await api.communityCatalog());
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy("");
    }
  }

  async function uninstall(item: RemoveTarget) {
    setBusy(`remove:${item.packageId}`);
    setMessage("");
    try {
      await api.uninstallCommunityPackage(item.packageId);
      setMessage(t(`“${item.name}” was uninstalled. Run history was preserved.`, `「${item.name}」已卸载，历史运行记录仍保留。`));
      setRemoveTarget(null);
      setCatalog(await api.communityCatalog());
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy("");
    }
  }

  const q = query.trim().toLowerCase();
  const items = useMemo(
    () =>
      catalog.items.filter((item) => {
        if (!q) return true;
        const text = [item.package.name, item.package.summary, item.package.author, ...item.package.tags]
          .join(" ")
          .toLowerCase();
        return text.includes(q);
      }),
    [catalog.items, q],
  );
  const unavailableInstalled = useMemo(
    () =>
      snap.tasks.filter(
        (task) =>
          task.community &&
          !catalog.items.some((item) => item.installedTaskId === task.id),
      ),
    [catalog.items, snap.tasks],
  );

  return (
    <div className="view community-view">
      <div className="view-head">
        <div>
          <h2 className="view-title">{t("Community", "插件中心")}</h2>
          <p className="view-subtitle">{t("Connect public registries and install community-maintained scripts and plugins", "连接公开注册表，安装社区维护的脚本与插件")}</p>
        </div>
        <span className="muted" />
        <button className="btn" onClick={() => void refresh()} disabled={loading || !sources.length}>
          <RefreshCw size={14} className={loading ? "spin" : ""} /> {t("Refresh", "刷新")}
        </button>
      </div>

      <div className="community-security" role="note">
        <ShieldAlert size={18} />
        <div>
          <strong>{t("Third-party code is not fully sandboxed", "第三方代码不会被完全隔离")}</strong>
          <p>{t("Installation verifies the source, SHA-256, package paths, and entry-point risk hints. Enabled code still runs with your Windows user permissions. Packages are installed disabled; review their source and permissions before enabling them.", "安装阶段只做来源、SHA-256、包路径校验和入口文本风险提示；启用后仍以你的 Windows 用户权限运行。安装后默认收纳且关闭，阅读源码和权限后再启用。")}</p>
        </div>
      </div>

      <section className="community-sources">
        <div className="community-section-head">
          <div>
            <strong>{t("Community sources", "社区源")}</strong>
            <span>{t("Public repositories can host a registry.json compatible with the Mosaic v1 protocol", "公开仓库可直接托管符合 Mosaic v1 协议的 registry.json")}</span>
          </div>
        </div>
        <div className="source-list">
          {sources.map((source) => (
            <div className="source-chip" key={source}>
              <Link2 size={13} />
              <span title={source}>{source}</span>
              <button
                className="icon-btn"
                aria-label={t(`Remove community source ${source}`, `移除社区源 ${source}`)}
                disabled={busy === "sources"}
                onClick={() => void persistSources(sources.filter((item) => item !== source))}
              >
                <X size={13} />
              </button>
            </div>
          ))}
          {!sources.length && <span className="muted small">{t("No community sources added.", "尚未添加社区源。")}</span>}
        </div>
        <div className="source-add">
          <input
            value={sourceInput}
            onChange={(event) => setSourceInput(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && sourceInput.trim()) void persistSources([...sources, sourceInput.trim()]);
            }}
            placeholder="https://example.org/mosaic/registry.json"
            aria-label={t("Community registry URL", "社区注册表 URL")}
          />
          <button
            className="btn"
            disabled={!sourceInput.trim() || busy === "sources"}
            onClick={() => void persistSources([...sources, sourceInput.trim()])}
          >
            <Plus size={14} /> {t("Add source", "添加源")}
          </button>
        </div>
      </section>

      {message && <div className="community-message" role="status">{message}</div>}
      {catalog.errors.map((error) => (
        <div className="form-error" key={error.sourceUrl}>
          <AlertTriangle size={14} />
          <span><b>{error.sourceUrl}</b>：{error.message}</span>
        </div>
      ))}

      {sources.length > 0 && (
        <div className="search-box community-search">
          <Search size={15} className="muted" />
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Search by name, author, or tag…", "搜索名称、作者或标签…")} />
        </div>
      )}

      <div className="community-grid">
        {items.map((item) => {
          const installed = Boolean(item.installedVersion);
          const current = item.installedVersion === item.package.version;
          const permissions = permissionLabels(item, locale);
          return (
            <article className="community-card" key={`${item.sourceUrl}:${item.package.id}:${item.package.version}`}>
              <div className="community-card-head">
                <div className="package-mark"><PackageCheck size={18} /></div>
                <div>
                  <h3>{item.package.name}</h3>
                  <p>{item.package.id} · v{item.package.version}</p>
                </div>
                {installed && <span className="installed-badge"><CheckCircle2 size={12} /> {current ? t("Installed", "已安装") : t(`Installed ${item.installedVersion}`, `已装 ${item.installedVersion}`)}</span>}
              </div>
              <p className="community-summary">{item.package.summary}</p>
              <div className="package-meta">
                <span>{item.package.author}</span>
                <span>{runtimeName(item.package.runtime, locale)}</span>
                <span>{item.package.kind === "plugin" ? t("Resident plugin", "常驻插件") : t("Script", "脚本")}</span>
                {item.package.license && <span>{item.package.license}</span>}
              </div>
              <div className="permission-list">
                {permissions.map((permission) => <span key={permission}>{permission}</span>)}
              </div>
              <div className="community-card-actions">
                <button
                  className={current ? "btn" : "btn primary"}
                  disabled={current || Boolean(busy)}
                  onClick={() => setInstallTarget(item)}
                >
                  <Download size={14} /> {installed ? current ? t("Up to date", "已是最新版") : t("Update", "更新") : t("Install", "安装")}
                </button>
                {installed && (
                  <button
                    className="btn danger"
                    disabled={Boolean(busy)}
                    onClick={() => setRemoveTarget({ packageId: item.package.id, name: item.package.name })}
                  >
                    <Trash2 size={13} /> {t("Uninstall", "卸载")}
                  </button>
                )}
              </div>
            </article>
          );
        })}
      </div>

      {unavailableInstalled.length > 0 && (
        <section className="unavailable-packages">
          <div className="community-section-head">
            <div>
              <strong>{t("Installed packages with unavailable sources", "来源不可用的已安装项目")}</strong>
              <span>{t("Local packages can still be stopped and removed when a source is removed or temporarily offline.", "即使社区源被移除或暂时失联，仍可安全停止并卸载本地包。")}</span>
            </div>
          </div>
          <div className="community-grid">
            {unavailableInstalled.map((task) => {
              const source = task.community!;
              return (
                <article className="community-card unavailable-card" key={task.id}>
                  <div className="community-card-head">
                    <div className="package-mark"><PackageCheck size={18} /></div>
                    <div>
                      <h3>{task.nickname}</h3>
                      <p>{source.packageId} · v{source.version}</p>
                    </div>
                    <span className="installed-badge"><AlertTriangle size={12} /> {t("Source unavailable", "来源不可用")}</span>
                  </div>
                  <p className="community-summary">{t("The registry was removed or cannot currently return this version. The local source record and hash are preserved.", "登记源已移除或当前无法返回此版本；本地安装来源和哈希仍保留。")}</p>
                  <div className="package-meta">
                    <span>{source.author}</span>
                    <span>{runtimeName(source.runtime, locale)}</span>
                    <span>{task.kind === "plugin" ? t("Resident plugin", "常驻插件") : t("Script", "脚本")}</span>
                  </div>
                  <div className="community-card-actions">
                    <button
                      className="btn danger"
                      disabled={Boolean(busy)}
                      onClick={() => setRemoveTarget({ packageId: source.packageId, name: task.nickname })}
                    >
                      <Trash2 size={13} /> {t("Stop and uninstall", "停止并卸载")}
                    </button>
                  </div>
                </article>
              );
            })}
          </div>
        </section>
      )}

      {!sources.length && (
        <div className="community-empty">
          <Link2 size={22} />
          <strong>{t("Add your first open-source community source", "添加第一个开源社区源")}</strong>
          <p>{t("The registry protocol and examples live in the source repository's ", "注册表协议与登记示例位于项目源码的 ")}<code>community/</code>{t(" directory. Once a source maintainer reviews a PR, users can install it here.", " 目录。源维护者审核 PR 后，用户即可在这里一键安装。")}</p>
        </div>
      )}
      {sources.length > 0 && !loading && !items.length && !catalog.errors.length && (
        <div className="empty">{t("No matching scripts or plugins were found in the community sources.", "社区源中还没有匹配的脚本或插件。")}</div>
      )}

      {installTarget && (
        <div className="modal" onClick={() => setInstallTarget(null)}>
          <div className="sheet" role="dialog" aria-modal="true" aria-labelledby="install-package-title" onClick={(event) => event.stopPropagation()}>
            <div className="sheet-head">
              <h3 id="install-package-title">{t(`Install “${installTarget.package.name}”`, `安装「${installTarget.package.name}」`)}</h3>
              <button className="icon-btn" onClick={() => setInstallTarget(null)} aria-label={t("Close", "关闭")}><X size={16} /></button>
            </div>
            <div className="install-facts">
              <span>{t("Author", "作者")} <b>{installTarget.package.author}</b></span>
              <span>{t("Runtime", "运行时")} <b>{runtimeName(installTarget.package.runtime, locale)}</b></span>
              <span>SHA-256 <code>{installTarget.package.sha256.slice(0, 16)}…</code></span>
            </div>
            <div className="warn-banner permanent-warning">
              <AlertTriangle size={15} />
              <span>{t("A matching hash only proves the download matches the registry; it does not prove the code is safe. Mosaic does not execute code during installation, and packages remain disabled afterward.", "哈希校验只能确认下载内容与登记一致，不能证明代码无恶意。Mosaic 不会在安装时执行代码，安装后也保持未启用。")}</span>
            </div>
            <div className="permission-review">
              <strong>{t("Permissions declared by the author", "作者声明的权限")}</strong>
              {permissionLabels(installTarget, locale).map((permission) => <span key={permission}>{permission}</span>)}
            </div>
            <div className="sheet-actions">
              <button className="btn" onClick={() => setInstallTarget(null)}>{t("Cancel", "取消")}</button>
              <button className="btn primary" disabled={Boolean(busy)} onClick={() => void install(installTarget)}>
                {busy ? t("Verifying and installing…", "校验并安装中…") : t("I understand the risk — install disabled", "理解风险，安装到收纳区")}
              </button>
            </div>
          </div>
        </div>
      )}

      {removeTarget && (
        <div className="modal" onClick={() => setRemoveTarget(null)}>
          <div className="sheet" role="alertdialog" aria-modal="true" aria-labelledby="remove-package-title" onClick={(event) => event.stopPropagation()}>
              <h3 id="remove-package-title">{t(`Uninstall “${removeTarget.name}”`, `卸载「${removeTarget.name}」`)}</h3>
            <p className="scan-summary">{t("This stops the plugin, removes its task and installed package files, and preserves run and output history.", "将停止插件、移除任务并删除已安装包文件；历史运行记录和产物记录会保留。")}</p>
            <div className="sheet-actions">
              <button className="btn" onClick={() => setRemoveTarget(null)}>{t("Cancel", "取消")}</button>
              <button className="btn danger" disabled={Boolean(busy)} onClick={() => void uninstall(removeTarget)}>
                {busy ? t("Uninstalling…", "卸载中…") : t("Confirm uninstall", "确认卸载")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
