[CmdletBinding()]
param(
    [string]$AppRoot = '',
    [string]$PackageRoot = '',
    [string]$EndpointStatePath = '',
    [string]$ReceiptPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$OffFormatGuid = '{00000000-0000-0000-0000-000000000000}'

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
    $ReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-provider-selection-check-last.json'
}

function Assert-Elevated {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Spatial-provider selection check requires an elevated Administrator PowerShell.'
    }
}

function Invoke-NativeCaptured {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)][string]$Label,
        [int[]]$AllowedExitCodes = @(0)
    )

    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $lines = @(& $Path @Arguments 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
        if ($null -eq $code) { $code = 0 }
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }

    $lines | ForEach-Object { Write-Host $_ }
    if ($AllowedExitCodes -notcontains [int]$code) {
        throw "$Label failed with exit code $code."
    }
    return [pscustomobject]@{
        code = [int]$code
        output = [string[]]$lines
    }
}

function Get-OutputValue {
    param(
        [Parameter(Mandatory = $true)][object]$Result,
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $prefix = $Name + [char]9
    $matches = @($Result.output | Where-Object { $_.StartsWith($prefix) })
    if ($matches.Count -ne 1) {
        throw "$Label did not emit exactly one $Name value."
    }
    return [string]$matches[0].Substring($prefix.Length)
}

function Require-Pattern {
    param(
        [Parameter(Mandatory = $true)][object]$Result,
        [Parameter(Mandatory = $true)][string]$Pattern,
        [Parameter(Mandatory = $true)][string]$Label
    )
    if ((@($Result.output) -join [Environment]::NewLine) -notmatch $Pattern) {
        throw "$Label did not satisfy required output pattern: $Pattern"
    }
}

Assert-Elevated

if ([Environment]::Is64BitOperatingSystem -and -not [Environment]::Is64BitProcess) {
    throw 'Spatial-provider selection check must run from a 64-bit PowerShell process on 64-bit Windows.'
}

$activationRunner = Join-Path $PSScriptRoot 'run-spatial-provider-activation-check.ps1'
$enableScript = Join-Path $PSScriptRoot 'Enable-OmniphonySpatialProvider.ps1'
$restartAudio = Join-Path $PSScriptRoot 'Restart-OmniphonyAudio.ps1'
$rootControl = Join-Path $PSScriptRoot 'OmniphonySpatialProbeCtl.exe'
$activationReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-provider-activation-check-last.json'
$liveReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-provider-live.json'

foreach ($path in @(
    $PackageRoot,
    $EndpointStatePath,
    $activationRunner,
    $enableScript,
    $restartAudio,
    $rootControl
)) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Required selection-check input is missing: $path"
    }
}

$endpointState = Get-Content -LiteralPath $EndpointStatePath -Raw | ConvertFrom-Json
$endpointId = [string]$endpointState.EndpointId
$endpointName = [string]$endpointState.EndpointName
if ([string]::IsNullOrWhiteSpace($endpointId)) {
    throw "Endpoint state does not contain EndpointId: $EndpointStatePath"
}

Write-Host 'Omniphony Spatial Sound first-selection check'
Write-Host "Endpoint: $endpointName"
Write-Host 'Safety boundary: this transaction only starts from Windows Spatial Sound Off.'
Write-Host 'Recovery boundary: after success, leave Omniphony selected until the explicit cleanup step.'
Write-Host ''

$runtimeBefore = Invoke-NativeCaptured -Path $rootControl -Arguments @('runtime-status') -Label 'Initial runtime status'
$runtimeBeforeText = @($runtimeBefore.output) -join [Environment]::NewLine
$runtimeKeyWasPresent = $false
$runtimeEndpointBefore = '<none>'
$runtimeDllBefore = '<none>'

if ($runtimeBeforeText -match 'SPATIAL_RUNTIME_STATUS\s+KEY=0\s+ENABLED=0\s+READY=0') {
    $runtimeKeyWasPresent = $false
}
elseif ($runtimeBeforeText -match 'SPATIAL_RUNTIME_STATUS\s+KEY=1\s+ENABLED=0\s+READY=0') {
    $runtimeKeyWasPresent = $true
    $runtimeEndpointBefore = Get-OutputValue -Result $runtimeBefore -Name 'SPATIAL_RUNTIME_ENDPOINT' -Label 'Initial runtime status'
    $runtimeDllBefore = Get-OutputValue -Result $runtimeBefore -Name 'SPATIAL_RUNTIME_REALTIME_DLL' -Label 'Initial runtime status'
    Require-Pattern -Result $runtimeBefore -Pattern 'SPATIAL_RUNTIME_REALTIME_DLL_EXISTS\s+1' -Label 'Initial runtime status'
}
else {
    throw 'Initial provider runtime state is not closed. Selection check refused.'
}

$selectionBefore = Invoke-NativeCaptured -Path $rootControl -Arguments @('selection-status', $endpointId) -Label 'Initial Windows spatial selection'
Require-Pattern -Result $selectionBefore -Pattern 'OMNIPHONY_DEFAULT\s+0' -Label 'Initial Windows spatial selection'
Require-Pattern -Result $selectionBefore -Pattern 'OMNIPHONY_ACTIVE\s+0' -Label 'Initial Windows spatial selection'
$defaultBefore = Get-OutputValue -Result $selectionBefore -Name 'DEFAULT_FORMAT' -Label 'Initial Windows spatial selection'
$activeBefore = Get-OutputValue -Result $selectionBefore -Name 'ACTIVE_FORMAT' -Label 'Initial Windows spatial selection'
if (-not $defaultBefore.Equals($OffFormatGuid, [System.StringComparison]::OrdinalIgnoreCase) -or
    -not $activeBefore.Equals($OffFormatGuid, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "First-selection check requires Spatial Sound Off before mutation. DEFAULT=$defaultBefore ACTIVE=$activeBefore"
}

$activationArgs = @{
    AppRoot = $AppRoot
    PackageRoot = $PackageRoot
    EndpointStatePath = $EndpointStatePath
    ReceiptPath = $activationReceiptPath
}
& $activationRunner @activationArgs

if (-not (Test-Path -LiteralPath $activationReceiptPath -PathType Leaf)) {
    throw "Activation receipt is missing: $activationReceiptPath"
}
$activationReceipt = Get-Content -LiteralPath $activationReceiptPath -Raw | ConvertFrom-Json
if ($activationReceipt.Pass -ne $true -or
    $activationReceipt.ProviderSelectionChanged -ne $false -or
    $activationReceipt.RollbackVerified -ne $true) {
    throw 'Immediate pre-selection activation proof did not pass.'
}

$stageManifestPath = Join-Path $AppRoot 'SpatialProvider\staged-generation.json'
if (-not (Test-Path -LiteralPath $stageManifestPath -PathType Leaf)) {
    throw "Stage manifest is missing: $stageManifestPath"
}
$stage = Get-Content -LiteralPath $stageManifestPath -Raw | ConvertFrom-Json
if ([string]$activationReceipt.Generation -ne [string]$stage.generation -or
    [string]$activationReceipt.PackageSha256 -ne [string]$stage.package_sha256) {
    throw 'Activation receipt and staged generation identity differ.'
}

$generationRoot = [string]$stage.generation_root
$providerDll = [string]$stage.provider_dll
$realtimeDll = [string]$stage.realtime_dll
$generationControl = Join-Path $generationRoot 'OmniphonySpatialProbeCtl.exe'
$generationSmoke = Join-Path $generationRoot 'OmniphonySpatialProbeSmoke.exe'
foreach ($path in @($generationRoot, $providerDll, $realtimeDll, $generationControl, $generationSmoke)) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Staged generation input is missing: $path"
    }
}

$enableArgs = @{
    ProviderDll = $providerDll
    ControlPath = $generationControl
    RealtimeDll = $realtimeDll
    RestartAudioPath = $restartAudio
    EndpointStatePath = $EndpointStatePath
    ReceiptPath = $liveReceiptPath
}
& $enableScript @enableArgs

$selectionAfter = Invoke-NativeCaptured -Path $generationControl -Arguments @('selection-status', $endpointId) -Label 'Selected Windows spatial state'
Require-Pattern -Result $selectionAfter -Pattern 'OMNIPHONY_DEFAULT\s+1' -Label 'Selected Windows spatial state'
Require-Pattern -Result $selectionAfter -Pattern 'OMNIPHONY_ACTIVE\s+1' -Label 'Selected Windows spatial state'

$runtimeSmoke = Invoke-NativeCaptured -Path $generationSmoke -Arguments @($providerDll, '--expect-runtime') -Label 'Selected provider runtime activation smoke'
Require-Pattern -Result $runtimeSmoke -Pattern 'SPATIAL_PROVIDER_STREAM_AVAILABLE\s+1' -Label 'Selected provider runtime activation smoke'
Require-Pattern -Result $runtimeSmoke -Pattern 'SPATIAL_PROVIDER_RUNTIME_ACTIVATION_OK\s+1' -Label 'Selected provider runtime activation smoke'
Require-Pattern -Result $runtimeSmoke -Pattern 'SPATIAL_PROVIDER_RUNTIME_STREAM_STARTED\s+0' -Label 'Selected provider runtime activation smoke'

$receiptRoot = Split-Path -Parent $ReceiptPath
if ($receiptRoot) {
    New-Item -ItemType Directory -Force -Path $receiptRoot | Out-Null
}
$receipt = [ordered]@{
    SchemaVersion = 1
    TimestampUtc = [DateTime]::UtcNow.ToString('o')
    Pass = $true
    State = 'selected-active-awaiting-manual-off'
    EndpointId = $endpointId
    EndpointName = $endpointName
    Generation = [string]$stage.generation
    PackageSha256 = [string]$stage.package_sha256
    GenerationRoot = $generationRoot
    ProviderDll = $providerDll
    RealtimeDll = $realtimeDll
    InitialDefaultSpatialFormat = $defaultBefore
    InitialActiveSpatialFormat = $activeBefore
    InitialRuntimeKeyPresent = $runtimeKeyWasPresent
    InitialRuntimeEndpoint = $runtimeEndpointBefore
    InitialRuntimeDll = $runtimeDllBefore
    SelectionVerified = $true
    RuntimeActivationVerified = $true
    RuntimeStreamStartedByCheck = $false
    AutomaticDeselectionSupported = $false
    ManualDeselectionTarget = 'Spatial Sound Off'
    LiveProviderReceipt = $liveReceiptPath
}
$receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $ReceiptPath -Encoding UTF8

Write-Host ''
Write-Host 'SPATIAL_PROVIDER_SELECTION_CHECK_OK 1'
Write-Host 'SPATIAL_PROVIDER_WINDOWS_DEFAULT 1'
Write-Host 'SPATIAL_PROVIDER_WINDOWS_ACTIVE 1'
Write-Host 'SPATIAL_PROVIDER_SELECTED_RUNTIME_ACTIVATION_OK 1'
Write-Host 'SPATIAL_PROVIDER_SELECTED_RUNTIME_STREAM_STARTED 0'
Write-Host 'SPATIAL_PROVIDER_MANUAL_DESELECTION_REQUIRED 1'
Write-Host "Receipt: $ReceiptPath"
Write-Host ''
Write-Host 'Next: while Omniphony remains selected, run the real static-object client:'
Write-Host '  & "C:\Program Files\Omniphony\support\run-real-static-object-check.ps1"'
Write-Host ''
Write-Host 'After that physical source check, set Spatial sound to Off and run cleanup:'
Write-Host '  & "C:\Program Files\Omniphony\support\run-spatial-provider-selection-cleanup.ps1"'
