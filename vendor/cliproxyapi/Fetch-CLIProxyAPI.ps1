[CmdletBinding()]
param(
    [string]$ProxyUrl = $(if ($env:MOSAIC_DOWNLOAD_PROXY) { $env:MOSAIC_DOWNLOAD_PROXY } else { 'http://127.0.0.1:61193' })
)

$ErrorActionPreference = 'Stop'
$vendorRoot = $PSScriptRoot
$manifest = Get-Content -Raw -LiteralPath (Join-Path $vendorRoot 'manifest.json') | ConvertFrom-Json
$cacheDir = Join-Path $vendorRoot 'cache'
$stageDir = Join-Path $vendorRoot 'stage'
$archivePath = Join-Path $cacheDir $manifest.archiveName
$checksumsPath = Join-Path $cacheDir 'checksums.txt'
$partialPath = "$archivePath.partial"

function Get-Sha256([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    try {
        $sha256 = [Security.Cryptography.SHA256]::Create()
        try { ([BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant() }
        finally { $sha256.Dispose() }
    } finally { $stream.Dispose() }
}

function Download-Verified([string]$Url, [string]$Destination) {
    $temporary = "$Destination.partial"
    Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
    $parameters = @{ Uri = $Url; OutFile = $temporary; UseBasicParsing = $true }
    if (-not [string]::IsNullOrWhiteSpace($ProxyUrl)) { $parameters.Proxy = $ProxyUrl }
    Invoke-WebRequest @parameters
    Move-Item -LiteralPath $temporary -Destination $Destination -Force
}

New-Item -ItemType Directory -Path $cacheDir -Force | Out-Null
if (-not (Test-Path -LiteralPath $checksumsPath)) {
    Download-Verified $manifest.checksumsUrl $checksumsPath
}
$officialLine = Get-Content -LiteralPath $checksumsPath | Where-Object { $_ -match "\s$([regex]::Escape($manifest.archiveName))$" } | Select-Object -First 1
if (-not $officialLine -or -not $officialLine.ToLowerInvariant().StartsWith($manifest.archiveSha256.ToLowerInvariant())) {
    throw 'The pinned CLIProxyAPI archive hash does not match the official checksums file.'
}

if ((-not (Test-Path -LiteralPath $archivePath)) -or (Get-Sha256 $archivePath) -ne $manifest.archiveSha256.ToLowerInvariant()) {
    Download-Verified $manifest.archiveUrl $archivePath
}
if ((Get-Sha256 $archivePath) -ne $manifest.archiveSha256.ToLowerInvariant()) {
    throw 'CLIProxyAPI archive SHA-256 verification failed.'
}

Remove-Item -LiteralPath $stageDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $stageDir -Force | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [IO.Compression.ZipFile]::OpenRead($archivePath)
try {
    foreach ($name in @($manifest.executableName, 'LICENSE', 'config.example.yaml')) {
        $entry = $archive.Entries | Where-Object { $_.FullName -eq $name } | Select-Object -First 1
        if (-not $entry) { throw "Required CLIProxyAPI release entry is missing: $name" }
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, (Join-Path $stageDir $name), $true)
    }
} finally {
    $archive.Dispose()
}
Copy-Item -LiteralPath (Join-Path $vendorRoot 'config.empty.yaml') -Destination (Join-Path $stageDir 'config.empty.yaml') -Force
if ((Get-Sha256 (Join-Path $stageDir $manifest.executableName)) -ne $manifest.executableSha256.ToLowerInvariant()) {
    throw 'CLIProxyAPI executable SHA-256 verification failed.'
}
if ((Get-Sha256 (Join-Path $stageDir 'config.example.yaml')) -ne $manifest.exampleConfigSha256.ToLowerInvariant()) {
    throw 'CLIProxyAPI official example configuration verification failed.'
}
if ((Get-Sha256 (Join-Path $stageDir 'LICENSE')) -ne $manifest.licenseSha256.ToLowerInvariant()) {
    throw 'CLIProxyAPI license verification failed.'
}
if ((Get-Sha256 (Join-Path $stageDir 'config.empty.yaml')) -ne $manifest.emptyConfigSha256.ToLowerInvariant()) {
    throw 'Mosaic empty CLIProxyAPI configuration verification failed.'
}
@(
    "Upstream: $($manifest.repository)",
    "Version: $($manifest.version)",
    "Archive: $($manifest.archiveName)",
    "Archive SHA-256: $($manifest.archiveSha256)",
    "Executable SHA-256: $($manifest.executableSha256)",
    'Runtime configuration: Mosaic credential-free template; no build-machine configuration included.',
    'Reference configuration: unmodified config.example.yaml from the verified official release.'
) | Set-Content -LiteralPath (Join-Path $stageDir 'PROVENANCE.txt') -Encoding utf8

$allowed = @('cli-proxy-api.exe', 'LICENSE', 'config.example.yaml', 'config.empty.yaml', 'PROVENANCE.txt')
$unexpected = Get-ChildItem -LiteralPath $stageDir -File | Where-Object { $_.Name -notin $allowed }
if ($unexpected -or (Get-ChildItem -LiteralPath $stageDir -File).Count -ne $allowed.Count) {
    throw 'CLIProxyAPI staging contains an unexpected or missing file.'
}

Write-Host "CLIProxyAPI $($manifest.version) staged from the verified official release."
