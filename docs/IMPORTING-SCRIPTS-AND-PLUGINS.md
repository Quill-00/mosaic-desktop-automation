# 脚本与插件导入指南

Mosaic 管理两类本地任务：执行完退出的“脚本”，以及保持运行直到用户关闭的“插件 / 常驻服务”。两者都以当前 Windows 用户权限运行。

## 从本地路径导入

1. 打开“脚本与插件”。
2. 点击“添加”。
3. 在“从本地项目 / 脚本导入”中粘贴完整路径。
4. 点击“识别入口”，检查自动填入的命令、参数、工作目录和产物目录。
5. 选择触发方式；保存前阅读真实源码扫描结果。

单个脚本文件会复制到 Mosaic 的应用数据目录，使用只含英文的受管目录名。项目目录不会复制，Mosaic 只记录原路径；移动项目后需要重新编辑任务。

## 自动识别

| 输入 | 默认命令 |
| --- | --- |
| `.py` / `.pyw` | `python` / `pythonw` |
| `.js` / `.mjs` / `.cjs` | `node` |
| `.ts` | `npx tsx` |
| `.ps1` | `powershell -NoProfile -ExecutionPolicy Bypass -File` |
| `.bat` / `.cmd` / `.exe` | 直接运行 |
| `.sh` / `.rb` | `bash` / `ruby` |
| Node 项目 | 按 lockfile 选择 pnpm、yarn 或 npm，运行 `start` 或 `dev` |
| Python 项目 | 优先 `.venv\Scripts\python.exe` 和 `main.py` / `app.py` |
| Rust 项目 | `cargo run` |

目录中名为 `start`、`run`、`main`、`fetch` 或 `collect` 的 PowerShell、批处理或 Python GUI 启动脚本优先于项目自动检测。

## 触发与生命周期

- 手动：用户点击运行。
- 间隔：每 N 秒运行一次。
- 每日、每周、每月：在指定日历时间运行。
- 文件看守：目录中的匹配文件变化后运行，内置防抖。
- 常驻插件：任务开关打开即启动；关闭时终止进程树。常驻任务不使用日历触发。

一次性任务应配置合理超时。常驻服务的超时为 0，由开关控制。

## stdout 输出协议

推荐打印一个 JSON 对象：

```json
{
  "summary": { "headline": "抓取 12 项", "count": 12 },
  "card": {
    "type": "news",
    "title": "更新",
    "items": [{ "title": "示例", "source": "本地脚本" }]
  },
  "items": [{ "title": "示例", "at": "2026-09-01T08:00:00Z" }],
  "cursor": "optional-next-cursor"
}
```

也支持：

- 直接打印带 `type` 的卡片对象；类型为 `list`、`news`、`metric`、`table` 或 `markdown`。
- 打印 JSON 数组，自动渲染为列表。
- 每行一条普通文本，作为列表兜底。

Mosaic 注入以下环境变量，帮助脚本做增量处理：

- `MOSAIC_LAST_RUN`：上次成功运行时间，RFC 3339。
- `MOSAIC_CURSOR`：上次输出中的 `cursor`。
- `MOSAIC_TASK_DIR`：该任务的持久状态目录。
- `MOSAIC_TASK_ID`：任务 ID。

交互式 CLI 可在任务的“交互输入”中按顺序每行填写一个答案；Mosaic 将它们写入 stdin。不要在这里保存密码或 token。

## 文件产物

任务设置“产物目录”后，详情页会列出目录中的最新文件。默认识别项目中的 `downloads`、`output`、`out`、`results` 或 `data` 目录。

产物可能包含个人数据。Mosaic 不上传它们，但用户在截图、Issue 或社区包中引用前必须自行脱敏。

## CLIProxyAPI 示例

全新安装会显示一个默认关闭的 `CLIProxyAPI` 常驻插件：

- 它不包含 CLIProxyAPI 二进制、配置、账号、Cookie 或 token。
- Windows 上优先寻找 `%USERPROFILE%\scoop\apps\cliproxyapi\<版本>\cli-proxy-api.exe`，避免通过 Scoop `current` junction 启动。
- 也可在启动 Mosaic 前设置 `CLIPROXYAPI_PATH` 为本机可执行文件的完整路径。
- 启用前请检查 CLIProxyAPI 自己的配置和监听地址；这些属于该工具，不属于 Mosaic 分发包。

## 从社区源安装

社区 ZIP 根目录必须包含 `mosaic-package.json`：

```json
{
  "schemaVersion": 1,
  "id": "example-task",
  "version": "1.0.0",
  "runtime": "node",
  "entry": "index.js"
}
```

支持 `node`、`python`、`powerShell` 和 `executable`。安装时 Mosaic 校验注册表、同源下载地址、SHA-256、包内清单、路径和体积；安装完成后任务仍默认关闭。

详细发布格式见 [community/README.md](../community/README.md)。

## 安全提醒

源码扫描只能识别一部分静态风险。它不会限制脚本读取文件、访问网络、修改注册表或启动其他进程。只导入你信任或审阅过的代码；不可信原生程序应放入 Windows Sandbox 或虚拟机运行。
