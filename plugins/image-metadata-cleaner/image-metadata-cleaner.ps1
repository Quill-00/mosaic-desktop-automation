[CmdletBinding()]
param(
    [ValidateSet('prefix', 'folder', 'overwrite')]
    [string]$Mode = 'prefix',
    [string]$Value = 'cleaned_',
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Paths
)

$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
[Console]::OutputEncoding = $utf8
$OutputEncoding = $utf8
$supportedExtensions = @('.jpg', '.jpeg', '.jfif', '.png', '.webp')

function Write-Bytes {
    param(
        [System.IO.Stream]$Stream,
        [byte[]]$Bytes,
        [int]$Offset,
        [int]$Count
    )
    $Stream.Write($Bytes, $Offset, $Count)
}

function Test-AsciiAt {
    param(
        [byte[]]$Bytes,
        [int]$Offset,
        [string]$Text
    )
    if ($Offset + $Text.Length -gt $Bytes.Length) { return $false }
    for ($index = 0; $index -lt $Text.Length; $index++) {
        if ($Bytes[$Offset + $index] -ne [byte][char]$Text[$index]) { return $false }
    }
    return $true
}

function Clear-JpegMetadata {
    param([byte[]]$Bytes)

    if ($Bytes.Length -lt 4 -or $Bytes[0] -ne 0xFF -or $Bytes[1] -ne 0xD8) {
        throw 'Invalid JPEG header.'
    }

    $output = [System.IO.MemoryStream]::new()
    Write-Bytes $output $Bytes 0 2
    $position = 2
    $finished = $false
    while ($position -lt $Bytes.Length) {
        $markerStart = $position
        if ($Bytes[$position] -ne 0xFF) { throw 'Invalid JPEG marker stream.' }
        while ($position -lt $Bytes.Length -and $Bytes[$position] -eq 0xFF) { $position++ }
        if ($position -ge $Bytes.Length) { throw 'Truncated JPEG marker.' }
        $marker = $Bytes[$position]
        $position++

        if ($marker -eq 0xD9) {
            Write-Bytes $output $Bytes $markerStart ($Bytes.Length - $markerStart)
            $finished = $true
            break
        }
        if ($marker -eq 0xD8 -or $marker -eq 0x01 -or ($marker -ge 0xD0 -and $marker -le 0xD7)) {
            Write-Bytes $output $Bytes $markerStart ($position - $markerStart)
            continue
        }
        if ($position + 2 -gt $Bytes.Length) { throw 'Truncated JPEG segment length.' }
        $segmentLength = ([int]$Bytes[$position] -shl 8) -bor [int]$Bytes[$position + 1]
        if ($segmentLength -lt 2 -or $position + $segmentLength -gt $Bytes.Length) {
            throw 'Invalid JPEG segment length.'
        }
        $segmentEnd = $position + $segmentLength

        if ($marker -eq 0xDA) {
            Write-Bytes $output $Bytes $markerStart ($Bytes.Length - $markerStart)
            $finished = $true
            break
        }

        $keep = $true
        if ($marker -eq 0xFE) {
            $keep = $false
        } elseif ($marker -ge 0xE0 -and $marker -le 0xEF) {
            $dataOffset = $position + 2
            $keep = ($marker -eq 0xE0 -and ((Test-AsciiAt $Bytes $dataOffset "JFIF`0") -or (Test-AsciiAt $Bytes $dataOffset "JFXX`0"))) -or
                $marker -eq 0xEE -or
                ($marker -eq 0xE2 -and (Test-AsciiAt $Bytes $dataOffset "ICC_PROFILE`0"))
        }
        if ($keep) {
            Write-Bytes $output $Bytes $markerStart ($segmentEnd - $markerStart)
        }
        $position = $segmentEnd
    }

    if (-not $finished) { throw 'JPEG scan data was not found.' }
    return $output.ToArray()
}

function Read-BigEndianUInt32 {
    param([byte[]]$Bytes, [int]$Offset)
    return ([uint32]$Bytes[$Offset] -shl 24) -bor
        ([uint32]$Bytes[$Offset + 1] -shl 16) -bor
        ([uint32]$Bytes[$Offset + 2] -shl 8) -bor
        [uint32]$Bytes[$Offset + 3]
}

function Clear-PngMetadata {
    param([byte[]]$Bytes)

    [byte[]]$signature = 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A
    if ($Bytes.Length -lt 20) { throw 'Invalid PNG header.' }
    for ($index = 0; $index -lt $signature.Length; $index++) {
        if ($Bytes[$index] -ne $signature[$index]) { throw 'Invalid PNG header.' }
    }

    $renderingChunks = @('IHDR', 'PLTE', 'IDAT', 'IEND', 'tRNS', 'cHRM', 'gAMA', 'iCCP', 'sRGB', 'cICP', 'mDCv', 'cLLi', 'sBIT', 'bKGD', 'acTL', 'fcTL', 'fdAT')
    $output = [System.IO.MemoryStream]::new()
    Write-Bytes $output $Bytes 0 8
    $position = 8
    $foundEnd = $false
    while ($position + 12 -le $Bytes.Length) {
        $length = Read-BigEndianUInt32 $Bytes $position
        if ($length -gt [int]::MaxValue) { throw 'PNG chunk is too large.' }
        $totalLength = 12 + [int]$length
        if ($position + $totalLength -gt $Bytes.Length) { throw 'Truncated PNG chunk.' }
        $type = [System.Text.Encoding]::ASCII.GetString($Bytes, $position + 4, 4)
        $isCritical = $type[0] -ge 'A' -and $type[0] -le 'Z'
        if ($isCritical -or $renderingChunks -contains $type) {
            Write-Bytes $output $Bytes $position $totalLength
        }
        $position += $totalLength
        if ($type -eq 'IEND') {
            $foundEnd = $true
            break
        }
    }
    if (-not $foundEnd) { throw 'PNG end chunk was not found.' }
    return $output.ToArray()
}

function Write-LittleEndianUInt32 {
    param([System.IO.Stream]$Stream, [uint32]$Value)
    [byte[]]$bytes = [BitConverter]::GetBytes($Value)
    Write-Bytes $Stream $bytes 0 4
}

function Clear-WebpMetadata {
    param([byte[]]$Bytes)

    if ($Bytes.Length -lt 12 -or -not (Test-AsciiAt $Bytes 0 'RIFF') -or -not (Test-AsciiAt $Bytes 8 'WEBP')) {
        throw 'Invalid WebP header.'
    }

    $body = [System.IO.MemoryStream]::new()
    $position = 12
    while ($position + 8 -le $Bytes.Length) {
        $type = [System.Text.Encoding]::ASCII.GetString($Bytes, $position, 4)
        $length = [BitConverter]::ToUInt32($Bytes, $position + 4)
        if ($length -gt [int]::MaxValue) { throw 'WebP chunk is too large.' }
        $paddedLength = [int]$length + ([int]$length % 2)
        $totalLength = 8 + $paddedLength
        if ($position + $totalLength -gt $Bytes.Length) { throw 'Truncated WebP chunk.' }
        if ($type -ne 'EXIF' -and $type -ne 'XMP ') {
            if ($type -eq 'VP8X' -and $length -ge 1) {
                [byte[]]$chunk = New-Object byte[] $totalLength
                [Array]::Copy($Bytes, $position, $chunk, 0, $totalLength)
                $chunk[8] = $chunk[8] -band 0xF3
                Write-Bytes $body $chunk 0 $chunk.Length
            } else {
                Write-Bytes $body $Bytes $position $totalLength
            }
        }
        $position += $totalLength
    }
    if ($position -ne $Bytes.Length) { throw 'Invalid WebP padding.' }

    $output = [System.IO.MemoryStream]::new()
    [byte[]]$riff = [System.Text.Encoding]::ASCII.GetBytes('RIFF')
    [byte[]]$webp = [System.Text.Encoding]::ASCII.GetBytes('WEBP')
    Write-Bytes $output $riff 0 4
    Write-LittleEndianUInt32 $output ([uint32](4 + $body.Length))
    Write-Bytes $output $webp 0 4
    $body.Position = 0
    $body.CopyTo($output)
    return $output.ToArray()
}

function Get-OutputPath {
    param(
        [string]$Source,
        [string]$Mode,
        [string]$Value
    )
    $directory = [IO.Path]::GetDirectoryName($Source)
    $fileName = [IO.Path]::GetFileName($Source)
    $stem = [IO.Path]::GetFileNameWithoutExtension($Source)
    $extension = [IO.Path]::GetExtension($Source)

    if ($Mode -eq 'overwrite') { return $Source }
    if ($Mode -eq 'folder') {
        $folderName = if ([string]::IsNullOrWhiteSpace($Value)) { '清洗后' } else { $Value.Trim() }
        if ($folderName.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0) { throw '子目录名称包含无效字符。' }
        $outputDirectory = Join-Path $directory $folderName
        [IO.Directory]::CreateDirectory($outputDirectory) | Out-Null
        return Join-Path $outputDirectory $fileName
    }

    $prefix = if ([string]::IsNullOrWhiteSpace($Value)) { 'cleaned_' } else { $Value }
    if ($prefix.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0) { throw '文件名前缀包含无效字符。' }
    $candidate = Join-Path $directory ($prefix + $stem + $extension)
    $index = 2
    while (Test-Path -LiteralPath $candidate) {
        $candidate = Join-Path $directory ($prefix + $index + '_' + $stem + $extension)
        $index++
    }
    return $candidate
}

function Get-ImagePaths {
    param([string[]]$InputPaths)
    $images = [System.Collections.Generic.List[string]]::new()
    $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($path in $InputPaths) {
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            $fullPath = [IO.Path]::GetFullPath($path)
            if ($supportedExtensions -contains [IO.Path]::GetExtension($fullPath).ToLowerInvariant() -and $seen.Add($fullPath)) {
                $images.Add($fullPath)
            }
            continue
        }
        if (Test-Path -LiteralPath $path -PathType Container) {
            foreach ($file in Get-ChildItem -LiteralPath $path -File | Sort-Object Name) {
                if ($supportedExtensions -contains $file.Extension.ToLowerInvariant() -and $seen.Add($file.FullName)) {
                    $images.Add($file.FullName)
                }
            }
        }
    }
    return $images.ToArray()
}

function Invoke-ImageCleaning {
    param(
        [string[]]$Images,
        [string]$Mode = 'prefix',
        [string]$Value = 'cleaned_'
    )
    if ($Images.Count -eq 0) { throw 'No supported image files were provided.' }

    $items = [System.Collections.Generic.List[object]]::new()
    foreach ($image in $Images) {
        [byte[]]$sourceBytes = [IO.File]::ReadAllBytes($image)
        $extension = [IO.Path]::GetExtension($image).ToLowerInvariant()
        [byte[]]$cleanBytes = switch ($extension) {
            '.jpg' { Clear-JpegMetadata $sourceBytes; break }
            '.jpeg' { Clear-JpegMetadata $sourceBytes; break }
            '.jfif' { Clear-JpegMetadata $sourceBytes; break }
            '.png' { Clear-PngMetadata $sourceBytes; break }
            '.webp' { Clear-WebpMetadata $sourceBytes; break }
            default { throw "Unsupported image format: $extension" }
        }
        $output = Get-OutputPath $image $Mode $Value
        $temporary = $output + '.mosaic-partial'
        [IO.File]::WriteAllBytes($temporary, $cleanBytes)
        if ($Mode -eq 'overwrite') {
            [IO.File]::Replace($temporary, $output, $null)
        } else {
            if (Test-Path -LiteralPath $output) { Remove-Item -LiteralPath $output -Force }
            [IO.File]::Move($temporary, $output)
        }
        $items.Add([ordered]@{
            title = [IO.Path]::GetFileName($output)
            subtitle = if ($Mode -eq 'overwrite') { '原图已清洗' } else { '清洗副本已保存' }
            path = $output
            at = [DateTimeOffset]::Now.ToString('O')
        })
    }
    return $items.ToArray()
}

$images = @(Get-ImagePaths $Paths)
$items = @(Invoke-ImageCleaning $images $Mode $Value)
[ordered]@{
    summary = [ordered]@{
        headline = "已清洗 $($items.Count) 张图片"
        count = $items.Count
        note = '像素数据未重编码；提示词与隐私元数据已移除'
    }
    items = $items
} | ConvertTo-Json -Depth 5 -Compress
