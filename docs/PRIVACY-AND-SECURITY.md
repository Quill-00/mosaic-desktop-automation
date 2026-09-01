# Privacy and security

## No account or license gate

Mosaic Desktop Automation is MIT-licensed software. Official builds contain no login, membership, subscription, activation code, device binding, or gate copied from another product.

The update client accesses only Mosaic's public version metadata. Requests contain the current version and a fixed public application identifier that cannot access users, orders, subscriptions, or other applications. The client does not call license, login, subscription, or payment endpoints.

## Local data

Tasks, execution records, notifications, results, and public configuration live in `%APPDATA%\com.mosaic.desktop\db.json`. Imported scripts and community packages stay inside Mosaic's application data directory.

User-entered secrets such as QQ AppSecret are stored through the system keychain in Windows Credential Manager. UI snapshots expose only whether a credential is configured, never the plaintext value.

The bundled CLIProxyAPI example is isolated from every global CPA installation. The release build includes only a hash-pinned official executable and license, the unmodified official example configuration, provenance, and Mosaic's credential-free template. Runtime state lives under Mosaic's own application-data directory, is created only when absent, and is launched through an explicit `--config` path. Mosaic never reads, copies, uploads, or overwrites global CPA configuration, OAuth data, or login credentials; release builds do not fall back to Scoop or another machine-wide CPA.

Mosaic has no telemetry upload. User tasks can access the network or filesystem when configured to do so.

## Update trust chain

- Version metadata must be signed by Mosaic's dedicated RSA private key; the client contains only its public key.
- Installer URLs must belong to this project's GitHub Releases.
- Downloads use `.partial` files, strict redirect and timeout handling, and a 256 MB limit.
- Length, PE headers, and SHA-256 are checked before staging.
- SHA-256 is checked again on the next launch before Inno Setup starts.
- Update failure never blocks the installed version.

## Third-party code

Community packages and local scripts run with the current Windows user's permissions. Permission declarations and source scans help users review risk but are not an operating-system sandbox.

Community installation requires HTTPS except for loopback development, same-origin package URLs, a 50 MB download limit, at most 512 files, and at most 128 MB extracted content. Mosaic rejects absolute paths, traversal, symlinks, hash mismatches, manifest mismatches, and missing entries. Installation never executes code, and new packages stay disabled.

## Public-release redaction

Source, issues, releases, screenshots, and packages must not contain credentials, cookies, sessions, private keys, runtime databases, logs, private absolute paths, personal desktop content, or Agent/Codex/Claude conversation history. README screenshots must contain only sanitized application data and no desktop background.

Report vulnerabilities through GitHub private vulnerability reporting as described in [SECURITY.md](../SECURITY.md). Never post credentials or user data in a public issue.
