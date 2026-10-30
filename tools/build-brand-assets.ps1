param([string]$Root = (Split-Path -Parent $PSScriptRoot))
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$Root = (Resolve-Path -LiteralPath $Root).Path
$source = [Drawing.Bitmap]::new((Join-Path $Root 'assets/logo-source.png'))
try {
    $left = $source.Width; $top = $source.Height; $right = 0; $bottom = 0
    for ($y = 0; $y -lt $source.Height; $y++) {
        for ($x = 0; $x -lt $source.Width; $x++) {
            if ($source.GetPixel($x, $y).A -ge 128) {
                $left = [Math]::Min($left, $x); $top = [Math]::Min($top, $y)
                $right = [Math]::Max($right, $x); $bottom = [Math]::Max($bottom, $y)
            }
        }
    }
    if ($right -le $left -or $bottom -le $top) { throw 'Logo is empty' }
    $bounds = [Drawing.Rectangle]::new($left, $top, $right-$left+1, $bottom-$top+1)
    function New-Logo([int]$Size) {
        $bitmap = [Drawing.Bitmap]::new($Size, $Size, [Drawing.Imaging.PixelFormat]::Format32bppPArgb)
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $ratio = ($Size * 0.84) / [Math]::Max($bounds.Width, $bounds.Height)
            $width = [int]($bounds.Width * $ratio); $height = [int]($bounds.Height * $ratio)
            $target = [Drawing.Rectangle]::new([int](($Size-$width)/2), [int](($Size-$height)/2), $width, $height)
            $graphics.DrawImage($source, $target, $bounds, [Drawing.GraphicsUnit]::Pixel)
        } finally { $graphics.Dispose() }
        return ,$bitmap
    }
    foreach ($entry in @(@(512,'assets/logo.png'), @(64,'assets/logo-64.png'), @(64,'site/favicon.png'), @(512,'site/logo.png'))) {
        $bitmap = New-Logo $entry[0]
        try { $bitmap.Save((Join-Path $Root $entry[1]), [Drawing.Imaging.ImageFormat]::Png) }
        finally { $bitmap.Dispose() }
    }
    $bitmap = New-Logo 96
    try {
        $rect = [Drawing.Rectangle]::new(0,0,96,96)
        $data = $bitmap.LockBits($rect, [Drawing.Imaging.ImageLockMode]::ReadOnly, [Drawing.Imaging.PixelFormat]::Format32bppPArgb)
        try {
            $pixels = [byte[]]::new(96*96*4)
            [Runtime.InteropServices.Marshal]::Copy($data.Scan0, $pixels, 0, $pixels.Length)
        } finally { $bitmap.UnlockBits($data) }
        $stream = [IO.File]::Create((Join-Path $Root 'assets/logo.bgra'))
        $writer = [IO.BinaryWriter]::new($stream)
        try { $writer.Write([Text.Encoding]::ASCII.GetBytes("SLBGRA1`0")); $writer.Write([uint32]96); $writer.Write([uint32]96); $writer.Write($pixels) }
        finally { $writer.Dispose() }
    } finally { $bitmap.Dispose() }
    $frames = @()
    foreach ($size in @(16,24,32,48,64,128,256)) {
        $bitmap = New-Logo $size; $stream = [IO.MemoryStream]::new()
        try { $bitmap.Save($stream,[Drawing.Imaging.ImageFormat]::Png); $frames += ,$stream.ToArray() }
        finally { $bitmap.Dispose(); $stream.Dispose() }
    }
    foreach ($name in @('secretarybird.ico','tray.ico')) {
        $writer = [IO.BinaryWriter]::new([IO.File]::Create((Join-Path $Root "assets/$name")))
        try {
            $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$frames.Count)
            $offset = 6 + 16*$frames.Count; $sizes = @(16,24,32,48,64,128,256)
            for ($i=0; $i -lt $frames.Count; $i++) {
                $dimension = if ($sizes[$i] -eq 256) {0} else {$sizes[$i]}
                $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
                $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]32)
                $writer.Write([uint32]$frames[$i].Length); $writer.Write([uint32]$offset)
                $offset += $frames[$i].Length
            }
            foreach ($frame in $frames) { $writer.Write([byte[]]$frame) }
        } finally { $writer.Dispose() }
    }
    $banner = [Drawing.Bitmap]::new(1600,440)
    $graphics = [Drawing.Graphics]::FromImage($banner)
    $mark = New-Logo 260
    $title = [Drawing.Font]::new('Segoe UI',64,[Drawing.FontStyle]::Bold,[Drawing.GraphicsUnit]::Pixel)
    $subtitle = [Drawing.Font]::new('Segoe UI',27,[Drawing.FontStyle]::Regular,[Drawing.GraphicsUnit]::Pixel)
    $white = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(238,241,238))
    $muted = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(147,157,147))
    try {
        $graphics.Clear([Drawing.Color]::FromArgb(14,17,14))
        $graphics.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
        $graphics.DrawImage($mark,100,90,260,260)
        $graphics.DrawString('SYNAPSE LITE',$title,$white,410,140)
        $graphics.DrawString('Your peripherals. Less overhead.',$subtitle,$muted,416,235)
        $banner.Save((Join-Path $Root 'docs/banner.png'),[Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose(); $banner.Dispose(); $mark.Dispose()
        $title.Dispose(); $subtitle.Dispose(); $white.Dispose(); $muted.Dispose()
    }
} finally { $source.Dispose() }
Write-Output 'Updated app, tray, web, README and native-tab branding.'
