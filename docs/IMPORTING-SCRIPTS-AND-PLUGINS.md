# Importing scripts and plugins

Mosaic manages two local task types: scripts that exit after completing work, and resident plugins that run until the user turns them off. Both run with the current Windows user's permissions.

## Import from a local path

1. Open **Scripts & plugins** and select **Add**.
2. Paste the full path under **Import from a local project or script**.
3. Select **Detect entry point**.
4. Review the detected command, arguments, working directory, and output folder.
5. Choose a trigger and review the real source scan before saving.

Single script files are copied into Mosaic's application data directory. Project folders remain referenced in place so their dependencies and data are not duplicated.

## Entry-point detection

| Input | Default command |
| --- | --- |
| `.py` / `.pyw` | `python` / `pythonw` |
| `.js` / `.mjs` / `.cjs` | `node` |
| `.ts` | `npx tsx` |
| `.ps1` | `powershell -NoProfile -ExecutionPolicy Bypass -File` |
| `.bat` / `.cmd` / `.exe` | Run directly |
| `.sh` / `.rb` | `bash` / `ruby` |
| Node project | Select pnpm, yarn, or npm from its lockfile; run `start` or `dev` |
| Python project | Prefer `.venv\Scripts\python.exe` and `main.py` or `app.py` |
| Rust project | `cargo run` |

## Triggers and lifecycle

- Manual, interval, daily, weekly, and monthly triggers.
- File watch with glob filtering and debounce.
- Resident plugins start when their switch is enabled and terminate their process tree when disabled.

Set a practical timeout for one-shot tasks. Resident tasks use a zero timeout and are controlled by their switch.

## Structured stdout

Printing one JSON object gives the richest result:

```json
{
  "summary": { "headline": "Fetched 12 items", "count": 12 },
  "card": {
    "type": "news",
    "title": "Updates",
    "items": [{ "title": "Example", "source": "Local script" }]
  },
  "items": [{ "title": "Example", "at": "2026-09-01T08:00:00Z" }],
  "cursor": "optional-next-cursor"
}
```

Mosaic also accepts a card object with `list`, `news`, `metric`, `table`, or `markdown` type; a JSON array; or plain line-oriented text.

Injected environment variables:

- `MOSAIC_LAST_RUN`: previous successful run time in RFC 3339.
- `MOSAIC_CURSOR`: the previous output's cursor.
- `MOSAIC_TASK_DIR`: persistent state directory for this task.
- `MOSAIC_TASK_ID`: task ID.

Interactive CLI answers can be provided one per line through the stdin field. Do not store passwords or tokens there.

## CLIProxyAPI example

Fresh installs include a disabled `CLIProxyAPI` resident-plugin entry and a pinned official Windows binary verified against the upstream SHA-256 checksum. The unmodified upstream `config.example.yaml` is included so users can see every supported option.

Mosaic does **not** import or inspect global CPA/Scoop configuration, OAuth files, cookies, accounts, or tokens. On first use it copies a credential-free template to `%APPDATA%\com.mosaic.desktop\cliproxyapi\config.yaml`, creates an adjacent private `auth` directory, and always starts the bundled CPA with an explicit `--config` path. If that Mosaic-local config already exists, upgrades never overwrite it. Development builds may use `CLIPROXYAPI_PATH` or Scoop only when no packaged resources exist; release builds never fall back to a global installation.

## CPA quota plugin (Mosaic 0.3.3)

The built-in CLIProxyAPI entry now includes a quota inspector. Open **Settings → CPA quota plugin**, enter the CPA base URL and **management key** (not an inference API key), then choose **Test & save**. The connection can also be configured directly inside the widget's quota panel. CPA management must already be enabled. Mosaic does not change CPA configuration or import credentials from a global installation.

In the desktop widget, click **CLIProxyAPI** to expand usage and reset details; they are collapsed initially and fold again when the widget closes. The separate power button still controls the local CPA process. A remote CPA connection can be inspected while the bundled local process is off.

- **Codex:** shared usage windows, code review and additional named/model windows returned by CPA. `used_percent` is treated as a percentage, including values below 1. Missing values remain unknown.
- **Antigravity:** model-group buckets and their reset timestamps; older `models.*.quotaInfo` responses are also accepted. Project identifiers come from account metadata or the read-only `loadCodeAssist` response; OAuth auth files are never downloaded.
- **Grok / xAI:** shared CLI credits and the monthly spending-cap balance. The billing rollover is labeled separately from quota reset. Product spending shares are not misrepresented as independent model limits. If a paid account does not expose billing data, it shows an explicit unavailable state; no paid chat request is sent as a fallback.

The panel shows local absolute reset times, countdowns, account errors, partial results and cached-data warnings. Reset deadlines do not automatically turn a cached zero balance into 100%. Open panels refresh every two minutes, with four concurrent account probes, request timeouts and a bounded batch window. Manual refresh is rate-limited to once per 15 seconds; very large pools can show unqueried accounts after the batch budget expires.

The management key and destination are stored together in the Windows credential store, never in `db.json`, task arguments or frontend storage. Changing the destination requires a new key. Disconnect removes this connection and its in-memory quota cache without modifying CPA accounts. Remote management connections require HTTPS and use Mosaic's local download proxy; localhost HTTP is supported. Provider requests originate at CPA, so configure CPA's own outbound proxy separately.

Protocol references inspected on 2026-09-03: [CPA Control Center](https://github.com/Biliniko/cpa-control-center), and the [CLIProxyAPI Management Center quota adapters](https://github.com/router-for-me/Cli-Proxy-API-Management-Center/tree/main/src/features/quota/providers). Integration uses `GET /v0/management/auth-files` and fixed provider quota requests through `POST /v0/management/api-call`. These provider APIs can change; unavailable data is reported explicitly.

## Community packages

Each ZIP must contain `mosaic-package.json` at its root:

```json
{
  "schemaVersion": 1,
  "id": "example-task",
  "version": "1.0.0",
  "runtime": "node",
  "entry": "index.js"
}
```

Supported runtimes are `node`, `python`, `powerShell`, and `executable`. Mosaic verifies the registry, same-origin package URL, SHA-256, manifest, paths, and size. Installed tasks remain disabled. See [the community registry guide](../community/README.md).

## Standard plugin surface

A plugin directory can include `mosaic-plugin.json`. Importing that directory detects the entry point without requiring users to edit command lines. The optional `ui` object declares a quick-widget drawer, accepted drop extensions, and action-menu arguments. Mosaic renders these capabilities generically; plugin-specific parameters and state remain in the plugin directory rather than Mosaic settings.

Use `schemaVersion: 1`, a stable `id`, `name`, `runtime`, `entry`, `kind: "plugin"`, and `lifecycle`. A drop-capable drawer declares `ui.drawer.drop.extensions`; dropped paths are appended to the plugin entry arguments. Drawer actions declare fixed argument arrays and are launched through the same audited task runner.

## Security reminder

Source scanning detects only some static risk patterns. It does not prevent scripts from reading files, using the network, modifying the registry, or starting other processes. Review code before importing it; run untrusted native programs in Windows Sandbox or a virtual machine.
