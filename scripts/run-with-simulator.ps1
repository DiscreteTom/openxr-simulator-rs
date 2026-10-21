[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Application,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Arguments,
    [string]$RuntimeRoot = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = 'Stop'
$manifest = Join-Path (Resolve-Path -LiteralPath $RuntimeRoot).Path 'openxr_simulator.json'
if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
    throw "Runtime manifest not found: $manifest"
}

$env:XR_RUNTIME_JSON = (Resolve-Path -LiteralPath $manifest).Path
& $Application @Arguments
exit $LASTEXITCODE
