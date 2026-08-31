# Contributing

Thanks for contributing code, documentation, translations, or community packages to Mosaic Desktop Automation.

## Before you start

1. Search existing issues to avoid duplicate work.
2. Explain the user scenario, security boundary, and verification plan for feature changes.
3. Preserve the local-first model, account-free access, and compatibility with existing task data.
4. Never commit internal design notes, Word documents, Agent conversations, runtime databases, logs, build output, credentials, private paths, or personal screenshots.

## Local checks

```powershell
pnpm build
cargo test --manifest-path .\src-tauri\Cargo.toml
```

For installer changes, also run `pnpm package:inno`. For community packages, increment the immutable package version, run `community\Build-Packages.ps1`, update SHA-256 in `community/registry.json`, and inspect the ZIP root.

## Pull requests

- Keep each pull request focused.
- Describe behavior changes, risks, and checks performed.
- Include sanitized application-window screenshots for UI changes; never include desktop backgrounds or personal data.
- Explain every new dependency and its license.
- Community packages must declare accurate network, filesystem, and channel capabilities.

Contributions are released under the project's [MIT License](LICENSE) and must follow the [Code of Conduct](CODE_OF_CONDUCT.md).
