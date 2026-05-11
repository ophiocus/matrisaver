param(
    [string]$PythonVersion = "3.12"
)

$ErrorActionPreference = "Stop"

Write-Host "Diagnostics:" -ForegroundColor Cyan
Get-Command python, py, pip -ErrorAction SilentlyContinue | Format-Table -AutoSize
try { python --version } catch { Write-Host "python not found" }
try { py --version } catch { Write-Host "py not found" }
try { py -0p } catch { Write-Host "py -0p not available" }
try { pip --version } catch { Write-Host "pip not found" }

Write-Host "" 
Write-Host "Ensuring Python $PythonVersion via winget..." -ForegroundColor Cyan
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Error "winget is not available. Install App Installer from Microsoft Store."
}

$pythonId = "Python.Python.$PythonVersion"
winget install --id $pythonId --source winget --silent --accept-package-agreements --accept-source-agreements

$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + `
    [System.Environment]::GetEnvironmentVariable("Path","User")

Write-Host "" 
Write-Host "Creating venv and installing requirements..." -ForegroundColor Cyan
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $repoRoot
py -$PythonVersion -m venv prototype\.venv
& .\prototype\.venv\Scripts\Activate.ps1
python -m pip install -r prototype\requirements.txt
Write-Host ""
Write-Host "Done. Activate with .\\prototype\\.venv\\Scripts\\Activate.ps1" -ForegroundColor Green
