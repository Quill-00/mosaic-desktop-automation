# Installation and updates

## Requirements

- Windows 10 or 11, x64.
- Microsoft Edge WebView2 Runtime. Supported Windows installations usually include it.
- Mosaic itself does not require Node.js. Node examples and imported Node scripts require Node.js 20 or newer.
- `CLIProxyAPI` is only a disabled example entry. Install that program separately if you want to use it; Mosaic never distributes its binary, configuration, or credentials.

## Install a release

Download the installer and matching checksum from [GitHub Releases](https://github.com/Quill-00/mosaic-desktop-automation/releases/latest), then verify them:

```powershell
$installer = '.\Mosaic-Setup-0.3.1.exe'
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

Do not attach these directories to issues. Share only the smallest manually redacted reproduction.

## Automatic updates

Mosaic accepts an update only when its version is newer, metadata signature is valid, SHA-256 is complete, and the download URL belongs to this project's GitHub Release path.

The client downloads into a `.partial` file, enforces redirect and size limits, verifies length, Windows PE headers, and SHA-256, then stages the installer atomically. It verifies SHA-256 again on the next launch before starting Inno Setup, before any window, script, bot, or plugin starts.

If GitHub is unavailable, Mosaic keeps the current version running, deletes incomplete downloads, and lets the user retry from **Settings → Automatic updates** or install manually from Releases. Update failure is never an access gate.

## Uninstall

Use **Windows Settings → Apps → Installed apps**. The uninstaller preserves `%APPDATA%\com.mosaic.desktop` by default so a reinstall can recover user tasks. Delete that exact application data directory and `%LOCALAPPDATA%\Mosaic\Updates` manually only after quitting Mosaic and confirming the paths.
