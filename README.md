<p align="center">
  <img src="assets/brand/mosaic-icon.svg" width="88" height="88" alt="Mosaic gradient circle icon">
</p>

# Mosaic Desktop Automation

> Windows 本地优先的脚本与插件自动化仪表盘。定时、看守或手动运行任务，把命令行结果变成可浏览的卡片、指标和时间轴。

[![MIT License](https://img.shields.io/badge/license-MIT-0a84ff.svg)](LICENSE)
[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11-0a84ff.svg)](docs/INSTALLATION.md)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24c8db.svg)](https://tauri.app/)
[![GitHub Release](https://img.shields.io/github/v/release/Quill-00/mosaic-desktop-automation?display_name=tag)](https://github.com/Quill-00/mosaic-desktop-automation/releases)

Mosaic 是一个面向 Windows 的 local-first script runner、scheduler、file watcher 和 plugin dashboard。它不要求登录账号，不设置会员、激活码、设备绑定或授权门槛；任务与结果保存在本机。

![Mosaic 主界面](docs/images/mosaic-main-window.png)

## 它能做什么

- 定时运行：间隔、每日、每周、每月和手动触发。
- 文件看守：监听目录变化，支持 glob 过滤与防抖。
- 常驻插件：随 Mosaic 启停本地服务，并在“正在运行”中管理进程。
- 结果仪表盘：把 JSON、JSON 数组或逐行文本渲染为列表、新闻、指标、表格与 Markdown 卡片。
- 产物时间轴：展示脚本输出和指定产物目录中的最新文件。
- 社区源：从 HTTPS 静态注册表发现包；安装前校验 SHA-256、路径和体积，新包默认关闭。
- 桌面悬浮界面：快速查看和启停常用脚本或插件。
- 安全自动更新：安装包只从 GitHub Releases 下载，验证版本元数据签名、PE 格式和 SHA-256 后，在下次启动时由 Inno Setup 安装。

![Mosaic 悬浮界面](docs/images/mosaic-widget.png)

## 安装

1. 打开 [GitHub Releases](https://github.com/Quill-00/mosaic-desktop-automation/releases/latest)。
2. 下载 `Mosaic-Setup-<版本>.exe` 和同名 `.sha256`。
3. 在 PowerShell 中校验后运行安装程序：

```powershell
$installer = '.\Mosaic-Setup-0.3.0.exe'
$actual = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
$expected = (Get-Content "$installer.sha256" -Raw).Split(' ', [System.StringSplitOptions]::RemoveEmptyEntries)[0].ToLowerInvariant()
if ($actual -ne $expected) { throw 'SHA-256 不匹配，请删除文件并重新下载。' }
```

命令无报错才可继续安装。完整系统要求、卸载和更新说明见 [安装指南](docs/INSTALLATION.md)。

## 首次启动会看到什么

全新用户数据中只预置不含个人信息的公开示例：

- Hacker News 热门、公开加密行情、北京天气和每日一言：使用公开 HTTP API，不带 API key、Cookie 或账号。
- `CLIProxyAPI`：只预置一个默认关闭的常驻插件入口；不捆绑程序、不捆绑配置，也不包含任何登录凭据。Mosaic 会寻找本机 Scoop 安装或 `CLIPROXYAPI_PATH` 指定的可执行文件。
- 官方社区源：预置 `Hello Mosaic` 与无需密钥的 Open-Meteo 天气脚本；安装后仍默认关闭。

这些示例可以删除或停用。它们的代码和网络目标都能在任务详情中检查。

## 导入自己的脚本或插件

在“脚本与插件 → 添加”中粘贴脚本文件或项目目录的完整路径，点击“识别入口”。Mosaic 能识别 Python、Node.js、TypeScript、PowerShell、批处理、Shell、Ruby、Rust 项目和本地 EXE。

单个脚本会复制到 Mosaic 的本机数据目录；完整项目保持原位引用，避免复制依赖和数据。触发、stdout 协议、常驻插件和社区包格式见 [脚本与插件导入指南](docs/IMPORTING-SCRIPTS-AND-PLUGINS.md)。

最小 stdout：

```json
{
  "summary": { "headline": "3 项完成", "count": 3 },
  "card": { "type": "metric", "title": "今日任务", "metrics": [{ "label": "完成", "value": "3" }] },
  "items": [{ "title": "生成日报", "at": "2026-09-01T08:00:00Z" }]
}
```

## 社区源

官方源随应用预置：

```text
https://raw.githubusercontent.com/Quill-00/mosaic-desktop-automation/main/community/registry.json
```

它是普通静态 JSON；安装包与注册表位于同一个 GitHub Raw 源，不依赖中心下载服务器。格式、作者发布流程和安全边界见 [Mosaic Community Registry](community/README.md)。

## 隐私与安全边界

- 无账号、无遥测、无广告、无许可校验；Mosaic 不上传本机任务、结果、路径或凭据。
- 版本检查只发送当前版本和固定的 Mosaic 公开应用标识；安装包字节全部来自 GitHub Releases。
- QQ AppSecret 等用户填写的渠道密钥存入 Windows 凭据管理器，不进入 `db.json`。
- 第三方脚本以当前 Windows 用户权限运行。静态扫描是风险提示，不是沙箱，也不能替代源码审查。
- GitHub 无法连接时，Mosaic 会提示“无法连接到 GitHub 更新，请检查网络”；当前版本继续可用，未完成的下载会被删除。

发布与威胁边界详见 [隐私与安全说明](docs/PRIVACY-AND-SECURITY.md)。

## 从源码构建

需要 Node.js 20+、pnpm、Rust stable、Visual Studio C++ Build Tools、WebView2；生成 Inno 安装包还需要 Inno Setup 6。

```powershell
pnpm install
pnpm build
cargo test --manifest-path .\src-tauri\Cargo.toml
pnpm tauri dev
pnpm package:inno
```

`pnpm package:inno` 会先构建和测试，再生成安装程序及 `.sha256`。构建输出位于本机忽略目录，不进入源码仓库。

## 技术栈

- Tauri 2 / Rust：调度、看守、进程树、社区包校验、更新与本机持久化。
- React 19 / TypeScript / Vite：主界面和桌面悬浮界面。
- Inno Setup 6：Windows 安装、升级和卸载。

## 参与贡献

欢迎提交 bug、功能建议、文档和社区包。请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 与 [SECURITY.md](SECURITY.md)。发布任何文件前都必须确认不含凭据、私人路径、运行数据库、日志、截图背景或对话历史。

## License

[MIT](LICENSE) © 2026 Mosaic contributors
