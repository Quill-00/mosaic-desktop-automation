[CmdletBinding()]
param(
    [switch]$SkipTests
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$manifestPath = Join-Path $repoRoot 'src-tauri\Cargo.toml'
$tauriConfigPath = Join-Path $repoRoot 'src-tauri\tauri.conf.json'
$packagePath = Join-Path $repoRoot 'package.json'
$isccPath = 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'
$stageDir = Join-Path $PSScriptRoot 'stage'
$outputDir = Join-Path $PSScriptRoot 'output'
$releaseExe = Join-Path $repoRoot 'src-tauri\target\release\mosaic.exe'
$stagedExe = Join-Path $stageDir 'Mosaic.exe'
$previousCargoOffline = $env:CARGO_NET_OFFLINE
$previousEncodedRustFlags = $env:CARGO_ENCODED_RUSTFLAGS
$previousRustFlags = $env:RUSTFLAGS

if (-not (Test-Path -LiteralPath $isccPath)) {
    throw "Inno Setup 6 was not found at $isccPath"
}

$package = Get-Content -Raw -LiteralPath $packagePath | ConvertFrom-Json
$tauriConfig = Get-Content -Raw -LiteralPath $tauriConfigPath | ConvertFrom-Json
$cargoText = Get-Content -Raw -LiteralPath $manifestPath
$cargoVersionMatch = [regex]::Match($cargoText, '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"')
if (-not $cargoVersionMatch.Success) {
    throw 'Unable to read the Cargo package version.'
}
$versions = @(@([string]$package.version, [string]$tauriConfig.version, $cargoVersionMatch.Groups[1].Value) | Select-Object -Unique)
if ($versions.Count -ne 1) {
    throw "Version mismatch: package.json=$($package.version), tauri.conf.json=$($tauriConfig.version), Cargo.toml=$($cargoVersionMatch.Groups[1].Value)"
}
$appVersion = $versions[0]

Push-Location $repoRoot
try {
    if (-not $SkipTests) {
        & pnpm build
        if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }
        & cargo test --offline --manifest-path $manifestPath
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    }

    # Rust dependencies can otherwise embed absolute Cargo/workspace source
    # paths in panic locations. Remap them without recording the builder's
    # username or checkout path in source control or release binaries.
    $releaseRustFlags = @()
    if (-not [string]::IsNullOrWhiteSpace($previousEncodedRustFlags)) {
        $releaseRustFlags += $previousEncodedRustFlags -split [char]0x1f
    }
    if (-not [string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
        $releaseRustFlags += "--remap-path-prefix=$($env:USERPROFILE)=C:\build-user"
    }
    $releaseRustFlags += "--remap-path-prefix=$repoRoot=C:\build\mosaic"
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
    $env:CARGO_ENCODED_RUSTFLAGS = $releaseRustFlags -join [char]0x1f
    $env:CARGO_NET_OFFLINE = 'true'
    & pnpm exec tauri build --no-bundle
    if ($LASTEXITCODE -ne 0) { throw 'Tauri release build failed.' }
} finally {
    if ($null -eq $previousCargoOffline) { Remove-Item Env:CARGO_NET_OFFLINE -ErrorAction SilentlyContinue } else { $env:CARGO_NET_OFFLINE = $previousCargoOffline }
    if ($null -eq $previousEncodedRustFlags) { Remove-Item Env:CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue } else { $env:CARGO_ENCODED_RUSTFLAGS = $previousEncodedRustFlags }
    if ($null -eq $previousRustFlags) { Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue } else { $env:RUSTFLAGS = $previousRustFlags }
    Pop-Location
}

if (-not (Test-Path -LiteralPath $releaseExe)) {
    throw "Release executable was not produced: $releaseExe"
}
New-Item -ItemType Directory -Force -Path $stageDir, $outputDir | Out-Null
Copy-Item -LiteralPath $releaseExe -Destination $stagedExe -Force

& $isccPath "/DSourceRoot=$stageDir" "/DAppVersion=$appVersion" "/DOutputDir=$outputDir" (Join-Path $PSScriptRoot 'Mosaic.iss')
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed.' }

$installerPath = Join-Path $outputDir "Mosaic-Setup-$appVersion.exe"
if (-not (Test-Path -LiteralPath $installerPath)) {
    throw "Installer was not produced: $installerPath"
}
$stream = [IO.File]::OpenRead($installerPath)
try {
    $sha256 = [Security.Cryptography.SHA256]::Create()
    try {
        $hashValue = ([BitConverter]::ToString($sha256.ComputeHash($stream))).Replace('-', '').ToLowerInvariant()
    } finally {
        $sha256.Dispose()
    }
} finally {
    $stream.Dispose()
}
$hashPath = "$installerPath.sha256"
"$hashValue  $([IO.Path]::GetFileName($installerPath))" | Set-Content -LiteralPath $hashPath -Encoding ascii

Write-Host "Installer: $installerPath"
Write-Host "SHA256:   $hashValue"
