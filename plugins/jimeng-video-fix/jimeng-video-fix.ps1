[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Paths
)

$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
[Console]::OutputEncoding = $utf8
$OutputEncoding = $utf8
$videoExtensions = @('.mp4', '.mov', '.m4v', '.mkv', '.webm')
$ffmpeg = Join-Path $PSScriptRoot 'ffmpeg.exe'

if (-not (Test-Path -LiteralPath $ffmpeg -PathType Leaf)) {
    throw "Missing bundled ffmpeg.exe: $ffmpeg"
}

$videos = [System.Collections.Generic.List[string]]::new()
$seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($path in $Paths) {
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        $fullPath = [IO.Path]::GetFullPath($path)
        if ($videoExtensions -contains [IO.Path]::GetExtension($fullPath).ToLowerInvariant() -and $seen.Add($fullPath)) {
            $videos.Add($fullPath)
        }
        continue
    }
    if (Test-Path -LiteralPath $path -PathType Container) {
        foreach ($file in Get-ChildItem -LiteralPath $path -File | Sort-Object Name) {
            if ($videoExtensions -contains $file.Extension.ToLowerInvariant() -and $seen.Add($file.FullName)) {
                $videos.Add($file.FullName)
            }
        }
    }
}

if ($videos.Count -eq 0) {
    throw 'No supported video files were provided.'
}

$outputSuffix = -join @([char]0x5373, [char]0x68A6, [char]0x4FEE, [char]0x590D)
$items = [System.Collections.Generic.List[object]]::new()
foreach ($video in $videos) {
    $directory = [IO.Path]::GetDirectoryName($video)
    $stem = [IO.Path]::GetFileNameWithoutExtension($video)
    $output = Join-Path $directory ($stem + '_' + $outputSuffix + '.mp4')
    $index = 2
    while (Test-Path -LiteralPath $output) {
        $output = Join-Path $directory ($stem + '_' + $outputSuffix + '-' + $index + '.mp4')
        $index++
    }

    & $ffmpeg -hide_banner -y -i $video `
        -map '0:v:0' -map '0:a?' `
        -map_metadata -1 -map_chapters -1 `
        -vf 'fps=24' `
        -c:v libx264 -preset slow -crf 12 -pix_fmt yuv420p `
        -c:a aac -b:a 128k -movflags +faststart `
        $output
    if ($LASTEXITCODE -ne 0) {
        throw "Video repair failed: $video"
    }
    $items.Add([ordered]@{
        title = [IO.Path]::GetFileName($output)
        subtitle = 'Saved beside the source video'
        path = $output
        at = [DateTimeOffset]::Now.ToString('O')
    })
}

[ordered]@{
    summary = [ordered]@{
        headline = "Repaired $($items.Count) video(s)"
        count = $items.Count
        note = 'Constant 24 FPS; no frame or duration cropping'
    }
    items = $items
} | ConvertTo-Json -Depth 5 -Compress
