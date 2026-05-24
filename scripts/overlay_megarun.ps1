# Overlay mega-run — headless batch render of a folder of images into
# their "matrix-code" overlay versions, with NO screen takeover.
#
# Points a custom overlay folder (sidecars on) at -ImageDir, moves the
# active variant's own queue dir aside so only these images cycle, then
# runs the host in headless benchmark mode long enough for every image
# to trigger, paint, and reach full bloom. The render-to-PNG feature
# writes <image>.overlay.png next to each source; this script collects
# them into -OutDir and reports coverage.
#
# Usage:
#   pwsh scripts/overlay_megarun.ps1 [-ImageDir <dir>] [-Variant <name>] [-OutDir <dir>]
#
# Defaults: ImageDir=scenes_dev, Variant=original, OutDir=artifacts/overlay_renders
param(
    [string]$ImageDir = "$PSScriptRoot\..\scenes_dev",
    [string]$Variant  = 'original',
    [string]$OutDir   = "$PSScriptRoot\..\artifacts\overlay_renders",
    [int]$Width  = 960,
    [int]$Height = 540
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path "$PSScriptRoot\.."
$exe  = Join-Path $root 'rust\target\release\matrisaver-host-windows.exe'
$ImageDir = (Resolve-Path $ImageDir).Path
if (-not (Test-Path $exe)) { throw "host not built: $exe (cargo build --release -p matrisaver-host-windows)" }

# Source images (skip our own .overlay.png outputs).
$exts = '.png','.jpg','.jpeg','.bmp','.webp'
$images = Get-ChildItem $ImageDir -File |
    Where-Object { $exts -contains $_.Extension.ToLower() -and $_.Name -notlike '*.overlay.png' -and $_.Name -notlike '*.mask.png' }
if ($images.Count -eq 0) { throw "no source images in $ImageDir" }
Write-Host "mega-run: $($images.Count) images from $ImageDir (variant=$Variant)"

# Clear stale outputs.
Get-ChildItem $ImageDir -Filter '*.overlay.png' -File -EA SilentlyContinue | Remove-Item -Force

# Temp settings: custom folder with sidecars on, short dwell.
$settings = [ordered]@{
    variant='original'; pipeline='opengl'; glow_quality='balanced'
    overlay_enabled=$true; performance_mode=$false; multi_monitor=$false; char_size=22
    overlay_directories=@([ordered]@{ path=$ImageDir; enabled=$true; write_ascii_alongside=$true })
    overlay_auto_levels=$false; overlay_persist_seconds=1.0
} | ConvertTo-Json -Depth 5
$settingsFile = Join-Path $env:TEMP 'matrisaver_megarun_settings.json'
Set-Content -Path $settingsFile -Value $settings -Encoding utf8

# Move the active variant's own overlay dir aside so only ImageDir cycles
# (avoids filename-dedup dropping same-named images).
$variantDir = Join-Path $root "assets\overlays\$Variant"
$variantBak = "$variantDir.__megarun_bak"
$moved = $false
if (Test-Path $variantDir) { Move-Item $variantDir $variantBak -Force; $moved = $true }

# Frames: ~8s initial + ~36s per cycle (active-hold + paint + dwell +
# random 15-30s re-trigger gap), at 60 fps, over-provisioned.
$frames = (($images.Count + 1) * 45) * 60

try {
    $env:MATRISAVER_SETTINGS_PATH = $settingsFile
    Write-Host "running headless benchmark: $frames frames @ ${Width}x${Height} ..."
    & $exe /s --gpu-scaffold --benchmark-frames $frames --variant $Variant --width $Width --height $Height 2>&1 |
        Select-String -Pattern 'PERF|ERROR|failed' | Select-Object -Last 3
}
finally {
    $env:MATRISAVER_SETTINGS_PATH = $null
    if ($moved) { Move-Item $variantBak $variantDir -Force }
}

# Collect renders + report coverage.
New-Item -ItemType Directory -Force $OutDir | Out-Null
Get-ChildItem $OutDir -Filter '*.overlay.png' -EA SilentlyContinue | Remove-Item -Force
$rendered = 0
foreach ($img in $images) {
    $sidecar = Join-Path $ImageDir ($img.Name + '.overlay.png')
    if (Test-Path $sidecar) {
        Copy-Item $sidecar (Join-Path $OutDir ($img.Name + '.overlay.png')) -Force
        $rendered++
        "  [ok]  $($img.Name)"
    } else {
        "  [MISS] $($img.Name)"
    }
}
Write-Host "mega-run done: $rendered/$($images.Count) rendered -> $OutDir"
