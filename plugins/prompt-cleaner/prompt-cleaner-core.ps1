[CmdletBinding()]
param(
    [switch]$SelfTest,
    [switch]$RunSaved
)

$ErrorActionPreference = 'Stop'
$script:Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[Console]::OutputEncoding = $script:Utf8NoBom
$OutputEncoding = $script:Utf8NoBom

function Get-TextFile {
    param([Parameter(Mandatory = $true)][string]$Path)

    $bytes = [IO.File]::ReadAllBytes($Path)
    $encoding = $null
    $offset = 0
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
        $encoding = New-Object System.Text.UTF8Encoding($true)
        $offset = 3
    } elseif ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFF -and $bytes[1] -eq 0xFE) {
        $encoding = New-Object System.Text.UnicodeEncoding($false, $true)
        $offset = 2
    } elseif ($bytes.Length -ge 2 -and $bytes[0] -eq 0xFE -and $bytes[1] -eq 0xFF) {
        $encoding = New-Object System.Text.UnicodeEncoding($true, $true)
        $offset = 2
    } else {
        try {
            $strictUtf8 = New-Object System.Text.UTF8Encoding($false, $true)
            [void]$strictUtf8.GetString($bytes)
            $encoding = New-Object System.Text.UTF8Encoding($false)
        } catch {
            $encoding = [Text.Encoding]::Default
        }
    }

    [pscustomobject]@{
        Text = $encoding.GetString($bytes, $offset, $bytes.Length - $offset)
        Encoding = $encoding
    }
}

function Set-TextFileAtomic {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text,
        [Parameter(Mandatory = $true)][Text.Encoding]$Encoding
    )

    $directory = [IO.Path]::GetDirectoryName($Path)
    $tempPath = Join-Path $directory ('.mosaic-prompt-cleaner-' + [Guid]::NewGuid().ToString('N') + '.tmp')
    try {
        [IO.File]::WriteAllText($tempPath, $Text, $Encoding)
        [IO.File]::Replace($tempPath, $Path, $null)
    } catch [PlatformNotSupportedException] {
        [IO.File]::Copy($tempPath, $Path, $true)
    } catch [IO.IOException] {
        [IO.File]::Copy($tempPath, $Path, $true)
    } catch [ArgumentException] {
        [IO.File]::Copy($tempPath, $Path, $true)
    } finally {
        if (Test-Path -LiteralPath $tempPath -PathType Leaf) {
            Remove-Item -LiteralPath $tempPath -Force
        }
    }
}

function Apply-LiteralRule {
    param(
        [AllowEmptyString()][string]$Text,
        [Parameter(Mandatory = $true)][string]$Find,
        [AllowEmptyString()][string]$Replacement,
        [bool]$IgnoreCase
    )

    if (-not $IgnoreCase) {
        return $Text.Replace($Find, $Replacement)
    }
    return [Text.RegularExpressions.Regex]::Replace(
        $Text,
        [Text.RegularExpressions.Regex]::Escape($Find),
        [Text.RegularExpressions.MatchEvaluator]{ param($match) $Replacement },
        [Text.RegularExpressions.RegexOptions]::IgnoreCase
    )
}

function Invoke-PromptCleaning {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][object[]]$Rules,
        [bool]$Recursive = $true
    )

    if (-not (Test-Path -LiteralPath $Root -PathType Container)) {
        throw '根目录不存在或不是文件夹。'
    }
    $validRules = @($Rules | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_.Find) })
    if ($validRules.Count -eq 0) {
        throw '请至少添加一条包含查找内容的规则。'
    }

    $files = if ($Recursive) {
        @(Get-ChildItem -LiteralPath $Root -Filter '*.txt' -File -Recurse)
    } else {
        @(Get-ChildItem -LiteralPath $Root -Filter '*.txt' -File)
    }
    $changed = 0
    $matches = 0
    foreach ($file in $files) {
        $source = Get-TextFile -Path $file.FullName
        $next = $source.Text
        foreach ($rule in $validRules) {
            $replacement = if ([string]$rule.Type -eq '删除') { '' } else { [string]$rule.Replace }
            $before = $next
            $next = Apply-LiteralRule -Text $next -Find ([string]$rule.Find) -Replacement $replacement -IgnoreCase ([bool]$rule.IgnoreCase)
            if ($next -ne $before) { $matches++ }
        }
        if ($next -ne $source.Text) {
            Set-TextFileAtomic -Path $file.FullName -Text $next -Encoding $source.Encoding
            $changed++
        }
    }

    [pscustomobject]@{
        Scanned = $files.Count
        Changed = $changed
        MatchedRules = $matches
    }
}

function Invoke-SelfTest {
    $root = Join-Path ([IO.Path]::GetTempPath()) ('mosaic-prompt-cleaner-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $root | Out-Null
    try {
        $nested = Join-Path $root 'nested'
        New-Item -ItemType Directory -Path $nested | Out-Null
        [IO.File]::WriteAllText((Join-Path $root 'a.txt'), 'bad, cat, BAD', (New-Object Text.UTF8Encoding($false)))
        [IO.File]::WriteAllText((Join-Path $nested 'b.txt'), 'cat and dog', (New-Object Text.UnicodeEncoding($false, $true)))
        $rules = @(
            [pscustomobject]@{ Type = '删除'; Find = 'bad'; Replace = ''; IgnoreCase = $true },
            [pscustomobject]@{ Type = '覆盖'; Find = 'cat'; Replace = 'tiger'; IgnoreCase = $false }
        )
        $result = Invoke-PromptCleaning -Root $root -Rules $rules -Recursive $true
        $a = (Get-TextFile -Path (Join-Path $root 'a.txt')).Text
        $b = (Get-TextFile -Path (Join-Path $nested 'b.txt')).Text
        if ($result.Scanned -ne 2 -or $result.Changed -ne 2 -or $a -ne ', tiger, ' -or $b -ne 'tiger and dog') {
            throw "Self-test mismatch: scanned=$($result.Scanned), changed=$($result.Changed), a=$a, b=$b"
        }
        [ordered]@{ ok = $true; scanned = $result.Scanned; changed = $result.Changed } | ConvertTo-Json -Compress
    } finally {
        Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

if ($RunSaved) {
    if (-not $env:MOSAIC_TASK_DIR) { throw 'MOSAIC_TASK_DIR is required.' }
    $runConfigPath = Join-Path $env:MOSAIC_TASK_DIR 'settings.json'
    if (-not (Test-Path -LiteralPath $runConfigPath -PathType Leaf)) { throw '尚未保存清洗配置。' }
    $runConfig = Get-Content -LiteralPath $runConfigPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $runRules = @($runConfig.rules | ForEach-Object {
        [pscustomobject]@{ Type = if ([string]$_.ruleType -eq 'replace') { '覆盖' } else { '删除' }; Find = [string]$_.find; Replace = [string]$_.replace; IgnoreCase = [bool]$_.ignoreCase }
    })
    $runResult = Invoke-PromptCleaning -Root ([string]$runConfig.root) -Rules $runRules -Recursive ([bool]$runConfig.recursive)
    [ordered]@{ scanned = $runResult.Scanned; changed = $runResult.Changed; matchedRules = $runResult.MatchedRules } | ConvertTo-Json -Compress
    exit 0
}

throw '请从 Mosaic 工作中心运行提示词清洗。'
