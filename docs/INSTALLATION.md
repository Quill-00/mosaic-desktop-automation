# 安装与更新

## 支持范围

- Windows 10 或 Windows 11，x64。
- Microsoft Edge WebView2 Runtime。大多数受支持的 Windows 已预装；缺少时请从微软官方安装。
- Mosaic 本体不依赖 Node.js。只有 Node 示例或用户导入的 Node 脚本需要 Node.js；建议 Node.js 20+。
- `CLIProxyAPI` 只是默认关闭的入口示例。要使用它，用户必须自行安装 CLIProxyAPI；Mosaic 不分发该程序或其凭据。

## 安装 Release

从 [GitHub Releases](https://github.com/Quill-00/mosaic-desktop-automation/releases/latest) 下载：

- `Mosaic-Setup-<版本>.exe`
- `Mosaic-Setup-<版本>.exe.sha256`

校验安装包：

```powershell
$installer = '.\Mosaic-Setup-0.3.0.exe'
$actual = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
$expected = (Get-Content "$installer.sha256" -Raw).Split(' ', [System.StringSplitOptions]::RemoveEmptyEntries)[0].ToLowerInvariant()
if ($actual -ne $expected) { throw 'SHA-256 不匹配，请删除文件并重新下载。' }
Start-Process -FilePath $installer
```

当前发布若尚未进行 Authenticode 代码签名，Windows SmartScreen 可能显示“未知发布者”。不要从第三方网盘或转载页面下载安装包；只使用本仓库 Release，并核对 SHA-256。

## 安装位置与用户数据

- 默认安装目录：`%LOCALAPPDATA%\Programs\Mosaic`
- 任务、结果与配置：`%APPDATA%\com.mosaic.desktop\db.json`
- 导入的单文件脚本：`%APPDATA%\com.mosaic.desktop\scripts\`
- 已下载、待下次启动安装的更新：`%LOCALAPPDATA%\Mosaic\Updates\`
- 渠道凭据：Windows 凭据管理器；不会写入 `db.json`。

不要把上述用户数据目录复制进源码仓库或问题报告。需要提供复现信息时，只提交手工脱敏后的最小内容。

## 自动更新

Mosaic 启动后检查自己的公开版本元数据。只有版本号更高、下载地址属于本项目的 GitHub Release、响应签名有效且 SHA-256 完整时，才会下载。

下载流程：

1. 仅接受 `https://github.com/Quill-00/mosaic-desktop-automation/releases/download/.../*.exe`。
2. 写入 `.partial` 临时文件，限制重定向和最大体积。
3. 校验完整长度、Windows PE 头和 SHA-256。
4. 原子保存安装包和待安装状态；当前进程绝不立即启动安装。
5. 下一次启动 Mosaic 时再次校验暂存文件 SHA-256；通过后才在创建窗口、脚本或插件进程之前启动 Inno Setup，然后退出旧进程。

如果中国大陆网络无法连接 GitHub，通知中心会显示“无法连接到 GitHub 更新，请检查网络”。这不是使用门限：当前版本继续运行，临时下载会被删除，可在“设置 → 自动更新”中重试，或手动从 Release 安装。

## 卸载

从 Windows“设置 → 应用 → 已安装的应用”卸载 Mosaic。卸载程序不会默认删除 `%APPDATA%\com.mosaic.desktop` 中的用户任务和结果，便于重装恢复。

如需彻底清除，请先退出 Mosaic，并确认目录路径准确后手工删除该应用专属数据目录和 `%LOCALAPPDATA%\Mosaic\Updates`。Windows 凭据管理器中的 Mosaic 渠道凭据应在应用内删除对应渠道，或由用户自行在凭据管理器中移除。
