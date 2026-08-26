[CmdletBinding()]
param(
    [string]$ControlPath = '',
    [string]$ReceiptPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Windows PowerShell 5.1 can evaluate script parameter defaults before
# $PSScriptRoot is populated. Resolve install-relative defaults only after the
# param block, matching the enable helper's compatibility boundary.
if ([string]::IsNullOrWhiteSpace($ControlPath)) {
    $ControlPath = Join-Path $PSScriptRoot 'OmniphonySpatialProbeCtl.exe'
}
if ([string]::IsNullOrWhiteSpace($ReceiptPath)) {
    $ReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-provider-last.json'
}

function Assert-Elevated {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Disable-OmniphonySpatialProvider.ps1 requires an elevated Administrator PowerShell.'
    }
}

Assert-Elevated

if (-not (Test-Path -LiteralPath $ControlPath -PathType Leaf)) {
    throw "Omniphony spatial provider control tool is missing: $ControlPath"
}
$ControlPath = (Resolve-Path -LiteralPath $ControlPath).Path

# Close/delete the application-stream gate through the single canonical owner
# before registration state is touched.
$previousPreference = $ErrorActionPreference
try {
    $ErrorActionPreference = 'Continue'
    $runtimeOutput = @(& $ControlPath runtime-disable 2>&1 | ForEach-Object { "$_" })
    $runtimeExit = $LASTEXITCODE
    if ($null -eq $runtimeExit) { $runtimeExit = 0 }
} finally {
    $ErrorActionPreference = $previousPreference
}
$runtimeOutput | ForEach-Object { Write-Host $_ }
if ($runtimeExit -ne 0) {
    throw "Omniphony spatial provider runtime-disable failed: exit=$runtimeExit"
}

$previousPreference = $ErrorActionPreference
try {
    $ErrorActionPreference = 'Continue'
    $output = @(& $ControlPath unregister 2>&1 | ForEach-Object { "$_" })
    $exitCode = $LASTEXITCODE
    if ($null -eq $exitCode) { $exitCode = 0 }
} finally {
    $ErrorActionPreference = $previousPreference
}
$output | ForEach-Object { Write-Host $_ }
if ($exitCode -ne 0) {
    throw "Omniphony spatial provider unregister failed: exit=$exitCode"
}

$stateRoot = Split-Path -Parent $ReceiptPath
if ($stateRoot) {
    New-Item -ItemType Directory -Force -Path $stateRoot | Out-Null
}
$receipt = [ordered]@{
    SchemaVersion = 1
    TimestampUtc = [DateTime]::UtcNow.ToString('o')
    Enabled = $false
    ProviderRegistered = $false
    NoMMDevicesWrites = $true
    SelectionChangedByScript = $false
}
$receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ReceiptPath -Encoding UTF8

Write-Host ''
Write-Host 'OMNIPHONY_SPATIAL_PROVIDER_ENABLED 0'
Write-Host 'Omniphony-owned Spatial\Encoder and COM registration were removed.'
Write-Host 'Windows MMDevices provider-selection state was not written by this script.'
Write-Host "Receipt: $ReceiptPath"
