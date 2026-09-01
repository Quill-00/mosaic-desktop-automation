[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$StageDir,
    [switch]$Installed
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$cpaDir = Join-Path $StageDir 'resources\cliproxyapi'
$manifest = Get-Content -Raw -LiteralPath (Join-Path $repoRoot 'vendor\cliproxyapi\manifest.json') | ConvertFrom-Json

function Get-Sha256([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    try {
        $sha256 = [Security.Cryptography.SHA256]::Create()
        try { ([BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant() }
        finally { $sha256.Dispose() }
    } finally { $stream.Dispose() }
}

function Assert-ExactNames([string]$Directory, [string[]]$Allowed) {
    $items = @(Get-ChildItem -LiteralPath $Directory -Force)
    $actual = @($items | ForEach-Object Name | Sort-Object)
    $expected = @($Allowed | Sort-Object)
    if (($actual -join "`n") -ne ($expected -join "`n")) {
        throw "Release staging contains unexpected or missing files in $Directory."
    }
}

$rootAllowed = @('Mosaic.exe', 'resources')
if ($Installed) { $rootAllowed += @('unins000.dat', 'unins000.exe') }
Assert-ExactNames $StageDir $rootAllowed
Assert-ExactNames (Join-Path $StageDir 'resources') @('cliproxyapi')
Assert-ExactNames $cpaDir @('cli-proxy-api.exe', 'LICENSE', 'config.example.yaml', 'config.empty.yaml', 'PROVENANCE.txt')

$hashes = @{
    'cli-proxy-api.exe' = $manifest.executableSha256
    'LICENSE' = $manifest.licenseSha256
    'config.example.yaml' = $manifest.exampleConfigSha256
    'config.empty.yaml' = $manifest.emptyConfigSha256
}
foreach ($entry in $hashes.GetEnumerator()) {
    if ((Get-Sha256 (Join-Path $cpaDir $entry.Key)) -ne $entry.Value.ToLowerInvariant()) {
        throw "Release staging hash mismatch: $($entry.Key)"
    }
}

$emptyConfig = Get-Content -Raw -LiteralPath (Join-Path $cpaDir 'config.empty.yaml')
if ($emptyConfig -notmatch '(?m)^api-keys:\s*\[\]\s*$' -or
    $emptyConfig -notmatch '(?m)^auth-dir:\s*"\./auth"\s*$' -or
    $emptyConfig -notmatch '(?m)^host:\s*"127\.0\.0\.1"\s*$') {
    throw 'The bundled CPA runtime template is not credential-free and loopback-only.'
}

$tracked = @(& git -C $repoRoot ls-files --cached --others --exclude-standard)
if ($LASTEXITCODE -ne 0 -or $tracked.Count -eq 0) {
    throw 'Unable to enumerate the exact Git upload set for the release audit.'
}
$forbiddenPaths = $tracked | Where-Object {
    $_ -match '(^|/)(\.env($|\.)|\.codex|\.agents|\.claude|logs?|credentials?|secrets?)(/|$)' -or
    $_ -match '\.(docx?|jsonl|pem|pfx|p12|key|log)$'
}
if ($forbiddenPaths) {
    throw "Forbidden private/runtime files are tracked:`n$($forbiddenPaths -join "`n")"
}

$highConfidenceSecrets = @(
    '-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----',
    'github_pat_[A-Za-z0-9_]{40,}',
    'gh[pousr]_[A-Za-z0-9]{36,}',
    'AKIA[0-9A-Z]{16}',
    'AIza[0-9A-Za-z_-]{30,}',
    'sk-[A-Za-z0-9_-]{24,}'
)
$privateUser = 'Co' + 'ven'
$personalMarkers = @(
    ('C:\Users\' + $privateUser),
    ('A:\' + 'Code\'),
    ('\Users\' + $privateUser),
    ($privateUser + '@')
)
$findings = [Collections.Generic.List[string]]::new()
foreach ($relative in $tracked) {
    $path = Join-Path $repoRoot ($relative -replace '/', '\')
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
    $item = Get-Item -LiteralPath $path
    if ($item.Length -gt 2MB) { continue }
    try { $text = Get-Content -Raw -LiteralPath $path -ErrorAction Stop } catch { continue }
    foreach ($pattern in $highConfidenceSecrets) {
        if ($text -match $pattern) { $findings.Add("$relative matches a credential pattern") }
    }
    foreach ($marker in $personalMarkers) {
        if ($text.IndexOf($marker, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            $findings.Add("$relative contains a private build-machine path")
        }
    }
}
if ($findings.Count -gt 0) {
    throw "Sensitive content detected in the Git upload set:`n$($findings -join "`n")"
}

$mosaicBytes = [IO.File]::ReadAllBytes((Join-Path $StageDir 'Mosaic.exe'))
$mosaicAscii = [Text.Encoding]::GetEncoding(28591).GetString($mosaicBytes)
$mosaicUtf16 = [Text.Encoding]::Unicode.GetString($mosaicBytes)
foreach ($marker in $personalMarkers) {
    if ($mosaicAscii.IndexOf($marker, [StringComparison]::OrdinalIgnoreCase) -ge 0 -or
        $mosaicUtf16.IndexOf($marker, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw 'The Mosaic executable contains a private build-machine marker.'
    }
}

Write-Host 'Release safety audit passed: exact CPA whitelist/hashes, empty isolated config, Git upload set, and builder-path scan.'
