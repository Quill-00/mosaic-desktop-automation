[CmdletBinding()]
param(
    [switch]$SelfTest,
    [switch]$RunSaved
)

$ErrorActionPreference = 'Stop'
$corePath = Join-Path $PSScriptRoot 'prompt-cleaner-core.ps1'
$source = [IO.File]::ReadAllText($corePath, [Text.Encoding]::UTF8)
$entry = [ScriptBlock]::Create($source)
& $entry -SelfTest:$SelfTest -RunSaved:$RunSaved
