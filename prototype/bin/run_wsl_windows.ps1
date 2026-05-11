param(
    [string]$Distro = "Ubuntu",
    [string]$RepoPath = ""
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Windows.Forms
$screens = [System.Windows.Forms.Screen]::AllScreens
$primary = $screens | Where-Object { $_.Primary } | Select-Object -First 1
if (-not $primary) {
    throw "Primary monitor not found."
}

$minX = ($screens | ForEach-Object { $_.Bounds.X } | Measure-Object -Minimum).Minimum
$minY = ($screens | ForEach-Object { $_.Bounds.Y } | Measure-Object -Minimum).Minimum
$maxX = ($screens | ForEach-Object { $_.Bounds.X + $_.Bounds.Width } | Measure-Object -Maximum).Maximum
$maxY = ($screens | ForEach-Object { $_.Bounds.Y + $_.Bounds.Height } | Measure-Object -Maximum).Maximum

$virtualWidth = $maxX - $minX
$virtualHeight = $maxY - $minY

$virtualBounds = "$minX,$minY,$virtualWidth,$virtualHeight"
$overlayRect = "$($primary.Bounds.X),$($primary.Bounds.Y),$($primary.Bounds.Width),$($primary.Bounds.Height)"
$windowPos = "$minX,$minY"
$monitorRects = ($screens | ForEach-Object { "$($_.Bounds.X),$($_.Bounds.Y),$($_.Bounds.Width),$($_.Bounds.Height)" }) -join ";"

if (-not $RepoPath) {
    $RepoPath = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
}

$repoWsl = (& $env:WINDIR\System32\wsl.exe -d $Distro -- wslpath -a "$RepoPath").Trim()

$envString = "MATRISAVER_VIRTUAL_BOUNDS='$virtualBounds' MATRISAVER_OVERLAY_RECT='$overlayRect' MATRISAVER_WINDOW_POS='$windowPos' MATRISAVER_MONITORS='$monitorRects'"
$command = "$envString '$repoWsl/prototype/bin/run.sh'"

& $env:WINDIR\System32\wsl.exe -d $Distro -- bash -lc $command
