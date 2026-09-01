<p align="center">
  <img src="assets/brand/mosaic-icon.svg" width="88" height="88" alt="Mosaic 圆形渐变图标">
</p>

# Mosaic Desktop Automation

> Windows 本地优先的脚本与插件自动化仪表盘。

[English](README.md) · [下载最新版](https://github.com/Quill-00/mosaic-desktop-automation/releases/latest)

Mosaic 可以定时、看守或手动运行脚本和常驻插件，并把结果展示成卡片、指标和时间轴。它不要求登录，不包含会员、激活码、设备绑定或许可门限，任务与结果保存在本机。

可在 **设置 → 窗口与显示** 中启用或关闭“登录 Windows 后自动启动 Mosaic”，无需管理员权限。

界面支持 English 与简体中文。首次启动时，UTC+8 时区自动使用中文，其他时区默认英文；手动选择后会记住偏好，并与桌面悬浮窗同步。

![Mosaic 主界面](docs/images/mosaic-main-window.png)

## 安装

1. 打开 [GitHub Releases](https://github.com/Quill-00/mosaic-desktop-automation/releases/latest)。
2. 下载 `Mosaic-Setup-<版本>.exe` 和同名 `.sha256`。
3. 使用 PowerShell 的 `Get-FileHash -Algorithm SHA256` 核对后运行安装程序。

安装包和自动更新流量均由 GitHub Releases 分发。无法连接 GitHub 时，Mosaic 会保留当前版本并删除未完成下载。

## 主要功能

- 间隔、每日、每周、每月、手动和文件看守触发。
- 本地脚本、完整项目、常驻插件和本地 EXE 导入。
- JSON、逐行文本、指标、新闻、表格和 Markdown 卡片。
- 运行历史、产物目录和桌面悬浮控制界面。
- HTTPS 社区源、SHA-256 校验和默认关闭安装策略。
- 无账号、无遥测、无广告、无许可校验。

首次数据只包含一个无需密钥的 Open-Meteo 天气示例，以及默认关闭的 `CLIProxyAPI`。CPA 可执行文件来自固定版本的官方 MIT Release，并通过 SHA-256 校验；安装包保留官方 `config.example.yaml` 供参考，实际运行使用复制到用户数据目录的独立空配置。构建过程不会读取或打包构建电脑上的 CPA 配置、OAuth 文件或登录凭据。

脚本和插件导入协议见 [Importing scripts and plugins](docs/IMPORTING-SCRIPTS-AND-PLUGINS.md)，隐私边界见 [Privacy and security](docs/PRIVACY-AND-SECURITY.md)。

## License

[MIT](LICENSE) © 2026 Mosaic contributors
