[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist ("openxr-simulator-windows-x64.stage-" + [Guid]::NewGuid().ToString('N'))
$zip = Join-Path $dist 'openxr-simulator-windows-x64.zip'

Push-Location $root
try {
    cargo build --release --lib
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }

    New-Item -ItemType Directory -Force -Path (Join-Path $stage 'scripts') | Out-Null

    Copy-Item -LiteralPath (Join-Path $root 'target\release\openxr_simulator.dll') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $root 'packaging\openxr_simulator.json') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $root 'packaging\settings.example.json') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $root 'scripts\register-runtime.ps1') -Destination (Join-Path $stage 'scripts')
    Copy-Item -LiteralPath (Join-Path $root 'scripts\unregister-runtime.ps1') -Destination (Join-Path $stage 'scripts')
    Copy-Item -LiteralPath (Join-Path $root 'scripts\run-with-simulator.ps1') -Destination (Join-Path $stage 'scripts')
    Copy-Item -LiteralPath (Join-Path $root 'README.md') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $stage

    if (Test-Path -LiteralPath $zip) {
        Remove-Item -LiteralPath $zip -Force
    }
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
    Write-Host "Created $zip" -ForegroundColor Green
} finally {
    if (Test-Path -LiteralPath $stage) {
        Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
    }
    Pop-Location
}
