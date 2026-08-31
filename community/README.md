# Mosaic Community Registry

Mosaic 社区源是一份通过 HTTPS 提供的静态 JSON 注册表。它不需要中心账户、登录或专用下载服务器；注册表与软件包可以直接托管在 GitHub。

官方源：

```text
https://raw.githubusercontent.com/Quill-00/mosaic-desktop-automation/main/community/registry.json
```

官方源当前只提供两个可审计的 MIT 示例：离线 `Hello Mosaic` 和无需 API key 的 Open-Meteo 天气卡片。安装后的任务默认关闭，必须由用户主动启用。

## 目录结构

```text
community/
├─ registry.json
├─ registry.schema.json
├─ package.schema.json
├─ Build-Packages.ps1
├─ examples/
│  └─ <package-id>/
│     ├─ mosaic-package.json
│     └─ <entry file>
└─ packages/
   └─ <package-id>-<version>.zip
```

每个 ZIP 的根目录必须直接包含 `mosaic-package.json`，不能再套一层目录。最小清单：

```json
{
  "schemaVersion": 1,
  "id": "hello-mosaic",
  "version": "1.0.0",
  "runtime": "node",
  "entry": "index.js"
}
```

支持的运行时为 `node`、`python`、`powerShell` 和 `executable`。

## 发布一个社区包

1. 在 `community/examples/<package-id>/` 中放入最少且可审计的源文件。
2. 确认不包含 `.env`、Cookie、token、账号、私钥、个人绝对路径、数据库、日志、产物或聊天记录。
3. 在仓库根目录运行：

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File .\community\Build-Packages.ps1
   ```

4. 将生成 ZIP 的 SHA-256 写入 `registry.json`，并声明最小权限。
5. 用 `registry.schema.json` 与 `package.schema.json` 校验 JSON，检查 ZIP 文件清单并运行示例。
6. 提交 Pull Request；安全相关问题请按仓库根目录的 `SECURITY.md` 私下报告。

注册表中的 `packageUrl` 必须是相对地址或与注册表同源的 HTTPS 地址。Mosaic 会拒绝跨源、明文 HTTP、哈希不符、路径穿越、清单不一致或体积超限的软件包。

## 权限声明

每个条目必须如实声明：

- `readPaths`：需要读取的路径范围。
- `writePaths`：需要写入的路径范围。
- `allowHosts`：需要访问的域名，不写协议或路径。
- `channels`：需要使用的 Mosaic 推送渠道。

权限声明用于让用户理解风险，并不是操作系统沙箱。社区脚本仍以当前 Windows 用户权限运行；维护者和用户都应阅读源码，不要把静态扫描当成安全保证。

## 第三方源

第三方维护者可以复制注册表结构并托管自己的 HTTPS 静态源。Mosaic 不为第三方源背书；添加源意味着信任该源今后提供的注册表和包。建议固定版本、保留源码、使用不可变发布资产，并在每次更新时重新审核权限和哈希。
