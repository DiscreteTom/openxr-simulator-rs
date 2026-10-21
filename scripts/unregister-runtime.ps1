[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$openxrDir = Join-Path $env:LOCALAPPDATA 'openxr\1'
$activeRuntime = Join-Path $openxrDir 'active_runtime.json'
$backup = Join-Path $openxrDir 'active_runtime.json.simulator-backup'

if (Test-Path -LiteralPath $backup -PathType Leaf) {
    Move-Item -LiteralPath $backup -Destination $activeRuntime -Force
    Write-Host 'Restored the previous OpenXR runtime registration.' -ForegroundColor Green
    return
}

if (Test-Path -LiteralPath $activeRuntime -PathType Leaf) {
    $active = Get-Content -LiteralPath $activeRuntime -Raw | ConvertFrom-Json
    $library = $active.runtime.library_path
    $leaf = if ($library) { Split-Path -Leaf $library } else { '' }
    # The json leaf also recognizes registrations written by version 0.1.0.
    if ($leaf -in @('openxr_simulator.dll', 'openxr_simulator.json')) {
        Remove-Item -LiteralPath $activeRuntime -Force
        Write-Host 'Removed the OpenXR Simulator registration.' -ForegroundColor Green
        Write-Host 'No previous runtime backup was available; install/select your headset runtime again if needed.' -ForegroundColor Yellow
        return
    }
}

Write-Host 'The active runtime is not OpenXR Simulator; nothing was changed.' -ForegroundColor Yellow
