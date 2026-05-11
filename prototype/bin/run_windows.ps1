param(
    [string]$Variant = "",
    [switch]$SingleDisplay
)

$ErrorActionPreference = "Stop"

function Show-Error([string]$message) {
    try {
        $logPath = Join-Path $env:TEMP "matrisaver-error.txt"
        $timestamp = Get-Date -Format "u"
        "$timestamp`r`n$message" | Set-Content -Path $logPath -Encoding UTF8
        Start-Process notepad.exe -ArgumentList $logPath | Out-Null
    } catch {
        Write-Error $message
    }
}

try {
    $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
    Set-Location $repoRoot

    if (-not (Test-Path prototype\.venv\Scripts\python.exe)) {
        throw "Missing prototype\.venv. Run setup_windows.ps1 first."
    }

    & .\prototype\.venv\Scripts\Activate.ps1

    $argsList = @("prototype\src\main.py")
    if ($Variant) {
        $argsList += "--variant"
        $argsList += $Variant
    }
    if ($SingleDisplay) {
        $argsList += "--single-display"
    }

    python @argsList
} catch {
    Show-Error $_.Exception.Message
    exit 1
}
