[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$community = Join-Path $root 'community'
$packages = Join-Path $community 'packages'
New-Item -ItemType Directory -Force -Path $packages | Out-Null

$entries = @(
    @{ Id = 'hello-mosaic'; Version = '1.0.0' },
    @{ Id = 'weather-open-meteo'; Version = '1.0.0' }
)

foreach ($entry in $entries) {
    $source = Join-Path $community "examples\$($entry.Id)"
    $archive = Join-Path $packages "$($entry.Id)-$($entry.Version).zip"
    if (Test-Path -LiteralPath $archive) {
        Remove-Item -LiteralPath $archive -Force
    }
    Compress-Archive -Path (Join-Path $source '*') -DestinationPath $archive -CompressionLevel Optimal
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Host "$($entry.Id) $hash"
}
