# Contributing to Mosaic

感谢为 Mosaic Desktop Automation 贡献代码、文档或社区包。

## 开始前

1. 先搜索现有 Issue，避免重复工作。
2. 功能变更请说明用户场景、安全边界和验证方式。
3. 保持本地优先、无账号门限和兼容现有任务数据。
4. 不要提交内部设计笔记、Word 文档、Agent 对话、运行数据库、日志、构建产物或个人截图。

## 本地检查

```powershell
pnpm install
pnpm build
cargo test --offline --manifest-path .\src-tauri\Cargo.toml
```

涉及安装器时，再运行：

```powershell
pnpm package:inno
```

涉及社区包时，运行 `community\Build-Packages.ps1`，更新 `community/registry.json` 中的 SHA-256，并检查 ZIP 根目录只有预期文件。

## Pull Request

- 聚焦一个问题，避免无关重构。
- 描述行为变化、风险和已运行检查。
- UI 变更附应用窗口截图；不得包含桌面背景或个人数据。
- 新依赖需解释用途与许可证。
- 新社区包必须声明准确的网络主机、文件读写和渠道能力。

提交即表示你的贡献按项目 [MIT License](LICENSE) 发布，并同意遵守 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。
