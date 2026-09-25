# 由一张源图生成全平台图标。
#
# 源图 app/icon-src.png：车 + 三张卡 + 问号，自带透明通道（四周本来就是空的）。
# 生成时先按 alpha 裁掉空白，再按比例铺进各尺寸的图标里：
#   · 旧版安卓图标（API < 26）+ Windows ico —— 黑底 + 图案（方的那枚裁圆一份）
#   · 自适应图标前景（API >= 26）—— 透明底 + 图案缩到画布 62%，落进 66dp 安全区
#   · 自适应图标背景 —— 纯黑（写进 res/values/colors.xml 的 ic_launcher_background）
#
# 缩小时走"逐级减半"再交给 GDI+ 双三次，避免 1200px 直接压到 48px 糊成一团。
#
# 用法：powershell -NoProfile -File app/make_icons.ps1
# 换图：把新图存成 app/icon-src.png 重跑；底色用 -BgHex 改（默认纯黑）。
# 依赖 .NET 的 System.Drawing（Windows 自带），不引第三方库。

param(
  [string]$Source = "$PSScriptRoot\icon-src.png",
  [string]$Ico = "$PSScriptRoot\src-tauri\icons\icon.ico",
  [string]$Png = "$PSScriptRoot\src-tauri\icons\icon.png",
  [string]$ResDir = "$PSScriptRoot\src-tauri\gen\android\app\src\main\res",
  [string]$BgHex = '#FF000000',
  [double]$LegacyFill = 0.86,     # 方形图标里图案占多少
  [double]$RoundFill = 0.82,      # 圆形图标（圆会切掉四角，留多一点）
  [double]$AdaptiveFill = 0.62    # 自适应前景占比（安全区）
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$bgColor = [System.Drawing.ColorTranslator]::FromHtml('#' + $BgHex.Substring(3))
$hex = '#FF{0:X2}{1:X2}{2:X2}' -f $bgColor.R, $bgColor.G, $bgColor.B

$src = [System.Drawing.Bitmap]::FromFile($Source)

# ---- 量出画面范围（源图四周是透明留白，不裁掉图案会显得很小）
$rect = [System.Drawing.Rectangle]::new(0, 0, $src.Width, $src.Height)
$data = $src.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly,
                      [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$stride = $data.Stride
$bytes = New-Object byte[] ($stride * $src.Height)
[System.Runtime.InteropServices.Marshal]::Copy($data.Scan0, $bytes, 0, $bytes.Length)
$src.UnlockBits($data)

# 先 4 像素一步粗扫，再在边界附近逐点精扫 —— 全图逐点扫在 PowerShell 里太慢
$minX = $src.Width; $minY = $src.Height; $maxX = -1; $maxY = -1
for ($y = 0; $y -lt $src.Height; $y += 4) {
  $base = $y * $stride
  for ($x = 0; $x -lt $src.Width; $x += 4) {
    if ($bytes[$base + $x * 4 + 3] -gt 8) {
      if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
      if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y }
    }
  }
}
if ($maxX -lt 0) { throw "源图整张都是透明的：$Source" }
for ($y = [Math]::Max(0, $minY - 5); $y -le [Math]::Min($src.Height - 1, $maxY + 5); $y++) {
  $base = $y * $stride
  for ($x = 0; $x -lt $src.Width; $x++) {
    if ($bytes[$base + $x * 4 + 3] -gt 8) {
      if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
      if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y }
    }
  }
}
$cropRect = [System.Drawing.Rectangle]::new($minX, $minY, $maxX - $minX + 1, $maxY - $minY + 1)
$crop = $src.Clone($cropRect, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
Write-Host "源图 $($src.Width)x$($src.Height) → 画面 $($cropRect.Width)x$($cropRect.Height)（左上 $minX,$minY），底色 $hex"

# ---- 缩放：逐级减半到目标的两倍以内，再让 GDI+ 收尾
$cache = @{}
function Get-Art([int]$targetLong) {
  if ($cache.ContainsKey($targetLong)) { return $cache[$targetLong] }
  $cur = $crop
  while ($cur.Width -gt $targetLong * 2) {
    $nw = [int][Math]::Max($targetLong, [Math]::Floor($cur.Width / 2))
    $nh = [int][Math]::Max(1, [Math]::Round($cur.Height * $nw / $cur.Width))
    $tmp = [System.Drawing.Bitmap]::new($nw, $nh, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($tmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($cur, 0, 0, $nw, $nh)
    $g.Dispose()
    if (-not [Object]::ReferenceEquals($cur, $crop)) { $cur.Dispose() }
    $cur = $tmp
  }
  $cache[$targetLong] = $cur
  return $cur
}

function New-IconBitmap([int]$size, [double]$fill, [bool]$round, [bool]$onBlack) {
  $bmp = [System.Drawing.Bitmap]::new($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.Clear([System.Drawing.Color]::Transparent)
  if ($round) {
    $clip = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $clip.AddEllipse(0, 0, $size - 1, $size - 1)
    $g.SetClip($clip)
  }
  if ($onBlack) { $g.Clear($bgColor) }
  $box = $size * $fill
  $art = Get-Art ([int]$box)
  $scale = $box / [Math]::Max($art.Width, $art.Height)
  $w = $art.Width * $scale
  $h = $art.Height * $scale
  $g.DrawImage($art, [System.Drawing.RectangleF]::new(
    [single](($size - $w) / 2), [single](($size - $h) / 2), [single]$w, [single]$h))
  $g.Dispose()
  return $bmp
}

function Save-Png([System.Drawing.Bitmap]$bmp, [string]$path) {
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
}

# 单帧 ico 的字节（借 Icon.Save 写 DIB 帧：小尺寸下比 PNG 帧兼容性好）
function Get-IcoFrame([System.Drawing.Bitmap]$bmp) {
  $h = $bmp.GetHicon()
  $icon = [System.Drawing.Icon]::FromHandle($h)
  $ms = [IO.MemoryStream]::new()
  $icon.Save($ms)
  $icon.Dispose()
  $b = $ms.ToArray(); $ms.Dispose()
  # 切片出来是 Object[]，BinaryWriter 只认 byte[]，必须显式转换
  [byte[]]$entry = $b[6..21]
  [byte[]]$payload = $b[22..($b.Length - 1)]
  return , @{ entry = $entry; data = $payload }
}

function Write-Ico([string]$path, [int[]]$sizes) {
  $frames = @()
  foreach ($s in $sizes) {
    $bmp = New-IconBitmap $s $LegacyFill $false $true
    $frames += , (Get-IcoFrame $bmp)
    $bmp.Dispose()
  }
  $out = [IO.MemoryStream]::new()
  $bw = [IO.BinaryWriter]::new($out)
  $bw.Write([UInt16]0); $bw.Write([UInt16]1); $bw.Write([UInt16]$frames.Count)
  $offset = 6 + 16 * $frames.Count
  foreach ($f in $frames) {
    # 帧头里的宽高/位深照抄 Icon.Save 的，只改数据偏移（后 8 字节）
    [byte[]]$head = $f.entry
    $bw.Write($head, 0, 8)
    $bw.Write([UInt32]$f.data.Length)
    $bw.Write([UInt32]$offset)
    $offset += $f.data.Length
  }
  foreach ($f in $frames) { [byte[]]$d = $f.data; $bw.Write($d, 0, $d.Length) }
  $bw.Flush()
  [IO.File]::WriteAllBytes($path, $out.ToArray())
  $bw.Dispose(); $out.Dispose()
  Write-Host "  ico  -> $path（$($sizes -join '/')）"
}

$bmp = New-IconBitmap 512 $LegacyFill $false $true
Save-Png $bmp $Png
$bmp.Dispose()
Write-Host "  预览 -> $Png（512px）"

Write-Ico $Ico @(16, 24, 32, 48, 64, 128, 256)

$densities = [ordered]@{ 'mdpi' = 1; 'hdpi' = 1.5; 'xhdpi' = 2; 'xxhdpi' = 3; 'xxxhdpi' = 4 }
foreach ($d in $densities.Keys) {
  $dir = Join-Path $ResDir "mipmap-$d"
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  $px = [int](48 * $densities[$d])
  $b = New-IconBitmap $px $LegacyFill $false $true; Save-Png $b (Join-Path $dir 'ic_launcher.png'); $b.Dispose()
  $b = New-IconBitmap $px $RoundFill $true $true; Save-Png $b (Join-Path $dir 'ic_launcher_round.png'); $b.Dispose()
  $b = New-IconBitmap ([int](108 * $densities[$d])) $AdaptiveFill $false $false
  Save-Png $b (Join-Path $dir 'ic_launcher_foreground.png'); $b.Dispose()
  Write-Host "  mipmap-$d -> $px px / 前景 $([int](108 * $densities[$d])) px"
}

$anydpi = Join-Path $ResDir 'mipmap-anydpi-v26'
New-Item -ItemType Directory -Force -Path $anydpi | Out-Null
$utf8 = New-Object Text.UTF8Encoding($false)
foreach ($n in @('ic_launcher', 'ic_launcher_round')) {
  $xml = @"
<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/ic_launcher_background" />
    <foreground android:drawable="@mipmap/ic_launcher_foreground" />
</adaptive-icon>
"@
  [IO.File]::WriteAllText((Join-Path $anydpi "$n.xml"), $xml, $utf8)
}

$colorsPath = Join-Path $ResDir 'values\colors.xml'
$colors = [IO.File]::ReadAllText($colorsPath)
if ($colors -match 'ic_launcher_background') {
  $colors = $colors -replace '<color name="ic_launcher_background">#[0-9A-Fa-f]+</color>', "<color name=`"ic_launcher_background`">$hex</color>"
} else {
  $colors = $colors -replace '</resources>', "    <color name=`"ic_launcher_background`">$hex</color>`r`n</resources>"
}
[IO.File]::WriteAllText($colorsPath, $colors, $utf8)
Write-Host "图标已生成（底色 $hex）"
