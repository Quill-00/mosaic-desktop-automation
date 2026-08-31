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

Fresh installs include a disabled `CLIProxyAPI` resident-plugin entry. It contains no binary, configuration, account, cookie, or token. On Windows, Mosaic looks for a physical Scoop installation path, or uses the full path supplied through `CLIPROXYAPI_PATH`.

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

## Security reminder

Source scanning detects only some static risk patterns. It does not prevent scripts from reading files, using the network, modifying the registry, or starting other processes. Review code before importing it; run untrusted native programs in Windows Sandbox or a virtual machine.
