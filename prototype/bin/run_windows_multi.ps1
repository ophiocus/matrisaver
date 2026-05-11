# Launch matrisaver on Windows with one instance per monitor.
# Pass -EnableOverlay to show the periodic ASCII art overlay.
param(
    [string]$RepoPath = "",
    [string]$PythonVersion = "3.12",
    [switch]$EnableOverlay
)

$ErrorActionPreference = "Stop"

function Write-ErrorLog([string]$message) {
    $timestamp = Get-Date -Format "u"
    $content = "$timestamp`r`n$message"
    for ($i = 0; $i -lt 5; $i++) {
        $logPath = Join-Path $env:TEMP ("matrisaver-windows-error-{0}.txt" -f ([guid]::NewGuid().ToString("N")))
        try {
            [System.IO.File]::WriteAllText($logPath, $content)
            return $logPath
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }
    return $null
}

function Show-Error([string]$message) {
    $logPath = Write-ErrorLog $message
    if ($logPath) {
        Write-Host "Error log written to $logPath"
        return
    }
    Write-Host $message
}

Add-Type -AssemblyName System.Windows.Forms
$screens = [System.Windows.Forms.Screen]::AllScreens

$monitorRects = ($screens | ForEach-Object { "$($_.Bounds.X),$($_.Bounds.Y),$($_.Bounds.Width),$($_.Bounds.Height)" }) -join ";"

if (-not $RepoPath) {
    $RepoPath = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
}

Set-Location $RepoPath

if (-not (Test-Path prototype\.venv\Scripts\python.exe)) {
    throw "Missing prototype\.venv. Run setup_windows.ps1 first."
}

try {
    $env:MATRISAVER_MONITORS = $monitorRects
    $env:MATRISAVER_PERFORMANCE = "1"
    & .\prototype\.venv\Scripts\Activate.ps1
    $stdoutPath = Join-Path $env:TEMP ("matrisaver-windows-output-{0}.txt" -f ([guid]::NewGuid().ToString("N")))
    $stderrPath = Join-Path $env:TEMP ("matrisaver-windows-error-{0}.err" -f ([guid]::NewGuid().ToString("N")))
    $pyArgs = @("-u", "prototype\\src\\main.py", "--performance")
    if ($EnableOverlay) { $pyArgs += "--enable-overlay" }
    $process = Start-Process -FilePath .\prototype\.venv\Scripts\python.exe -ArgumentList $pyArgs -NoNewWindow -PassThru -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
    Start-Sleep -Seconds 2
    if ($process.HasExited -and $process.ExitCode -ne 0) {
        $content = "Process exited with code $($process.ExitCode)."
        if (Test-Path $stdoutPath) {
            $content += "`r`n" + (Get-Content -Path $stdoutPath -Raw)
        }
        if (Test-Path $stderrPath) {
            $content += "`r`n" + (Get-Content -Path $stderrPath -Raw)
        }
        Show-Error $content
    }
} catch {
    Show-Error $_.Exception.Message
}
