[CmdletBinding()]
param(
    [string]$AppRoot = '',
    [string]$PackageRoot = '',
    [string]$EndpointStatePath = '',
    [string]$ReceiptPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($AppRoot)) {
    $AppRoot = Join-Path $env:ProgramFiles 'Omniphony'
}
if ([string]::IsNullOrWhiteSpace($PackageRoot)) {
    $PackageRoot = Join-Path $PSScriptRoot 'spatial-provider-dev'
}
if ([string]::IsNullOrWhiteSpace($EndpointStatePath)) {
    $EndpointStatePath = Join-Path $env:ProgramData 'Omniphony\endpoint-backup.json'
}
if ([string]::IsNullOrWhiteSpace($ReceiptPath)) {
    $ReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-provider-activation-check-last.json'
}

function Assert-Elevated {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Spatial-provider activation check requires an elevated Administrator PowerShell.'
    }
}

Assert-Elevated

if ([Environment]::Is64BitOperatingSystem -and -not [Environment]::Is64BitProcess) {
    throw 'Spatial-provider activation check must run from a 64-bit PowerShell process on 64-bit Windows.'
}

if (-not (Test-Path -LiteralPath $PackageRoot -PathType Container)) {
    throw "Spatial-provider package root is missing: $PackageRoot"
}
if (-not (Test-Path -LiteralPath $EndpointStatePath -PathType Leaf)) {
    throw "Endpoint state is missing: $EndpointStatePath"
}

$PackageRoot = (Resolve-Path -LiteralPath $PackageRoot).Path
$endpointState = Get-Content -LiteralPath $EndpointStatePath -Raw | ConvertFrom-Json
$endpointId = [string]$endpointState.EndpointId
$endpointName = [string]$endpointState.EndpointName
if ([string]::IsNullOrWhiteSpace($endpointId)) {
    throw "Endpoint state does not contain EndpointId: $EndpointStatePath"
}

$stageScript = Join-Path $PackageRoot 'Stage-OmniphonySpatialProvider.ps1'
if (-not (Test-Path -LiteralPath $stageScript -PathType Leaf)) {
    throw "Spatial-provider stage script is missing: $stageScript"
}

Write-Host 'Omniphony Spatial Sound activation-only check'
Write-Host "Endpoint: $endpointName"
Write-Host 'Safety boundary: provider selection will not be changed.'
Write-Host ''

& $stageScript -PackageRoot $PackageRoot -AppRoot $AppRoot

$stageManifestPath = Join-Path $AppRoot 'SpatialProvider\staged-generation.json'
if (-not (Test-Path -LiteralPath $stageManifestPath -PathType Leaf)) {
    throw "Stage manifest was not produced: $stageManifestPath"
}
$stage = Get-Content -LiteralPath $stageManifestPath -Raw | ConvertFrom-Json

$preflightScript = [string]$stage.preflight_script
$activationScript = [string]$stage.activation_test_script
foreach ($path in @($preflightScript, $activationScript)) {
    if ([string]::IsNullOrWhiteSpace($path) -or -not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Immutable activation-check script is missing: $path"
    }
}

$stateRoot = Join-Path $AppRoot 'SpatialProvider'
$preflightReportPath = Join-Path $stateRoot 'preflight-generation.json'
$activationReportPath = Join-Path $stateRoot 'activation-generation.json'

& $preflightScript -StageManifestPath $stageManifestPath -PhysicalEndpointId $endpointId -ReportPath $preflightReportPath
& $activationScript -StageManifestPath $stageManifestPath -PreflightReportPath $preflightReportPath -ReportPath $activationReportPath

$activation = Get-Content -LiteralPath $activationReportPath -Raw | ConvertFrom-Json
if ($activation.state -ne 'activation-proven-rolled-back-unselected' -or
    $activation.public_stream_available -ne $true -or
    $activation.public_stream_activated -ne $true -or
    $activation.public_stream_started -ne $false -or
    $activation.provider_selection_changed -ne $false -or
    $activation.rollback_verified -ne $true) {
    throw 'Activation check completed without the required rolled-back/unselected proof state.'
}

$receiptRoot = Split-Path -Parent $ReceiptPath
if ($receiptRoot) {
    New-Item -ItemType Directory -Force -Path $receiptRoot | Out-Null
}
$receipt = [ordered]@{
    SchemaVersion = 1
    TimestampUtc = [DateTime]::UtcNow.ToString('o')
    Pass = $true
    EndpointId = $endpointId
    EndpointName = $endpointName
    Generation = [string]$stage.generation
    PackageSha256 = [string]$stage.package_sha256
    StageManifest = $stageManifestPath
    PreflightReport = $preflightReportPath
    ActivationReport = $activationReportPath
    PublicStreamAvailable = $true
    PublicStreamActivated = $true
    PublicStreamStarted = $false
    ProviderSelectionChanged = $false
    RollbackVerified = $true
}
$receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ReceiptPath -Encoding UTF8

Write-Host ''
Write-Host 'SPATIAL_PROVIDER_ACTIVATION_CHECK_OK 1'
Write-Host 'SPATIAL_PROVIDER_SELECTION_CHANGED 0'
Write-Host 'SPATIAL_PROVIDER_ACTIVATION_ROLLBACK_VERIFIED 1'
Write-Host "Receipt: $ReceiptPath"
