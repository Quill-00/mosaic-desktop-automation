# Installation and updates

## Requirements

- Windows 10 or 11, x64.
- Microsoft Edge WebView2 Runtime. Supported Windows installations usually include it.
- Mosaic itself does not require Node.js. Node examples and imported Node scripts require Node.js 20 or newer.
- `CLIProxyAPI` is included as a disabled example from a pinned, SHA-256-verified official MIT release. The original upstream `config.example.yaml` is retained for learning, while Mosaic starts CPA only with its own credential-free per-user configuration.

## Install a release

Download the installer and matching checksum from [GitHub Releases](https://github.com/Quill-00/mosaic-desktop-automation/releases/latest), then verify them:

```powershell
$installer = '.\Mosaic-Setup-0.3.2.exe'
$actual = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
$expected = (Get-Content "$installer.sha256" -Raw).Split(' ', [System.StringSplitOptions]::RemoveEmptyEntries)[0].ToLowerInvariant()
if ($actual -ne $expected) { throw 'SHA-256 mismatch. Delete the installer and download it again.' }
Start-Process -FilePath $installer
```

Unsigned releases can trigger a Windows SmartScreen unknown-publisher warning. Download only from this repository and always verify SHA-256.

## Paths and local data

- Installation: `%LOCALAPPDATA%\Programs\Mosaic`
- Tasks, results, and public settings: `%APPDATA%\com.mosaic.desktop\db.json`
- Imported single-file scripts: `%APPDATA%\com.mosaic.desktop\scripts\`
- Staged updates: `%LOCALAPPDATA%\Mosaic\Updates\`
- Channel credentials: Windows Credential Manager, never `db.json`
- Mosaic CPA runtime: `%APPDATA%\com.mosaic.desktop\cliproxyapi\`; it is separate from global CPA/Scoop configuration and is created only if absent

Do not attach these directories to issues. Share only the smallest manually redacted reproduction.

## Automatic updates

Mosaic checks GitHub's public latest-release API directly and accepts an update only when its version is newer, both named assets belong to this project's GitHub Release path, and the matching `.sha256` file is complete and valid.

The client downloads into a `.partial` file, enforces redirect and size limits, verifies length, Windows PE headers, and SHA-256, then stages the installer atomically. It verifies SHA-256 again on the next launch before starting Inno Setup, before any window, script, bot, or plugin starts.

If GitHub is unavailable, Mosaic keeps the current version running, deletes incomplete downloads, and lets the user retry from **Settings → Automatic updates** or install manually from Releases. Update failure is never an access gate.

Mosaic-managed update and community-package downloads use `http://127.0.0.1:61193` as the default proxy. Advanced users can override it for the current process with `MOSAIC_DOWNLOAD_PROXY`; build-time CPA acquisition uses the same default.

## Launch at sign-in

Open **Settings → Windows & display** and enable **Launch Mosaic when I sign in to Windows**. Mosaic writes only the current user's `HKCU` startup entry, so administrator access is not required. Turn the switch off to remove that entry immediately.

## Uninstall

Use **Windows Settings → Apps → Installed apps**. The uninstaller preserves `%APPDATA%\com.mosaic.desktop` by default so a reinstall can recover user tasks. Delete that exact application data directory and `%LOCALAPPDATA%\Mosaic\Updates` manually only after quitting Mosaic and confirming the paths.
