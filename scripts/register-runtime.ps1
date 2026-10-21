[CmdletBinding()]
param(
    [string]$RuntimeRoot = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = 'Stop'
$RuntimeRoot = (Resolve-Path -LiteralPath $RuntimeRoot).Path
$manifest = Join-Path $RuntimeRoot 'openxr_simulator.json'
$library = Join-Path $RuntimeRoot 'openxr_simulator.dll'

if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
    throw "Runtime manifest not found: $manifest"
}
if (-not (Test-Path -LiteralPath $library -PathType Leaf)) {
    throw "Runtime library not found: $library"
}

$openxrDir = Join-Path $env:LOCALAPPDATA 'openxr\1'
$activeRuntime = Join-Path $openxrDir 'active_runtime.json'
$backup = Join-Path $openxrDir 'active_runtime.json.simulator-backup'
New-Item -ItemType Directory -Force -Path $openxrDir | Out-Null

$sameRuntime = $false
if (Test-Path -LiteralPath $activeRuntime) {
    try {
        $active = Get-Content -LiteralPath $activeRuntime -Raw | ConvertFrom-Json
        $currentLibrary = $active.runtime.library_path
        if ($currentLibrary) {
            $currentLibraryPath = [System.IO.Path]::GetFullPath($currentLibrary)
            $expectedLibraryPath = [System.IO.Path]::GetFullPath($library)
            $sameRuntime = $currentLibraryPath.Equals(
                $expectedLibraryPath,
                [StringComparison]::OrdinalIgnoreCase
            )
        }
    } catch {
        $sameRuntime = $false
    }
}

if (-not $sameRuntime -and (Test-Path -LiteralPath $activeRuntime) -and -not (Test-Path -LiteralPath $backup)) {
    Copy-Item -LiteralPath $activeRuntime -Destination $backup
}

$payload = [ordered]@{
    file_format_version = '1.0.0'
    runtime = [ordered]@{
        library_path = (Resolve-Path -LiteralPath $library).Path
        name = 'OpenXR Simulator'
    }
}
$payload | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $activeRuntime -Encoding utf8

Write-Host "Registered OpenXR Simulator:" -ForegroundColor Green
Write-Host "  $manifest"
Write-Host ''
Write-Host "For one process only, set:"
Write-Host "  `$env:XR_RUNTIME_JSON = '$((Resolve-Path -LiteralPath $manifest).Path)'"
