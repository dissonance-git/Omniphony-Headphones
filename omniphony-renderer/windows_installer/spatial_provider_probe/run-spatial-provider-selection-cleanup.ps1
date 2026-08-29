[CmdletBinding()]
param(
    [string]$EndpointStatePath = '',
    [string]$SelectionReceiptPath = '',
    [string]$CleanupReceiptPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$OffFormatGuid = '{00000000-0000-0000-0000-000000000000}'

if ([string]::IsNullOrWhiteSpace($EndpointStatePath)) {
    $EndpointStatePath = Join-Path $env:ProgramData 'Omniphony\endpoint-backup.json'
}
if ([string]::IsNullOrWhiteSpace($SelectionReceiptPath)) {
    $SelectionReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-provider-selection-check-last.json'
}
if ([string]::IsNullOrWhiteSpace($CleanupReceiptPath)) {
    $CleanupReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-provider-selection-cleanup-last.json'
}

function Assert-Elevated {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Spatial-provider selection cleanup requires an elevated Administrator PowerShell.'
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

$control = Join-Path $PSScriptRoot 'OmniphonySpatialProbeCtl.exe'
$disableScript = Join-Path $PSScriptRoot 'Disable-OmniphonySpatialProvider.ps1'
$endpointControl = Join-Path $PSScriptRoot 'OmniphonyEndpointCtl.exe'
$liveReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-provider-live.json'

foreach ($path in @(
    $control,
    $disableScript,
    $endpointControl,
    $EndpointStatePath,
    $SelectionReceiptPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required selection-cleanup input is missing: $path"
    }
}

$selectionReceipt = Get-Content -LiteralPath $SelectionReceiptPath -Raw | ConvertFrom-Json
if ($selectionReceipt.Pass -ne $true -or
    [string]$selectionReceipt.State -ne 'selected-active-awaiting-manual-off' -or
    $selectionReceipt.SelectionVerified -ne $true) {
    throw 'Selection receipt does not describe a proven live Omniphony selection.'
}

$endpointId = [string]$selectionReceipt.EndpointId
$endpointName = [string]$selectionReceipt.EndpointName
if ([string]::IsNullOrWhiteSpace($endpointId)) {
    throw 'Selection receipt does not identify an endpoint.'
}

$initialRuntimeEndpoint = '<none>'
$initialRuntimeDll = '<none>'
if ($selectionReceipt.InitialRuntimeKeyPresent -eq $true) {
    $initialRuntimeEndpoint = [string]$selectionReceipt.InitialRuntimeEndpoint
    $initialRuntimeDll = [string]$selectionReceipt.InitialRuntimeDll
    if ([string]::IsNullOrWhiteSpace($initialRuntimeEndpoint) -or
        [string]::IsNullOrWhiteSpace($initialRuntimeDll) -or
        -not (Test-Path -LiteralPath $initialRuntimeDll -PathType Leaf)) {
        throw 'Initial disabled runtime configuration cannot be restored exactly; cleanup refused before mutation.'
    }
}

Write-Host 'Omniphony Spatial Sound selection cleanup'
Write-Host "Endpoint: $endpointName"
Write-Host 'This step requires Windows Spatial Sound to already be Off.'
Write-Host ''

$selectionBeforeCleanup = Invoke-NativeCaptured -Path $control -Arguments @('selection-status', $endpointId) -Label 'Pre-cleanup Windows spatial selection'
Require-Pattern -Result $selectionBeforeCleanup -Pattern 'OMNIPHONY_DEFAULT\s+0' -Label 'Pre-cleanup Windows spatial selection'
Require-Pattern -Result $selectionBeforeCleanup -Pattern 'OMNIPHONY_ACTIVE\s+0' -Label 'Pre-cleanup Windows spatial selection'
$defaultBeforeCleanup = Get-OutputValue -Result $selectionBeforeCleanup -Name 'DEFAULT_FORMAT' -Label 'Pre-cleanup Windows spatial selection'
$activeBeforeCleanup = Get-OutputValue -Result $selectionBeforeCleanup -Name 'ACTIVE_FORMAT' -Label 'Pre-cleanup Windows spatial selection'
if (-not $defaultBeforeCleanup.Equals($OffFormatGuid, [System.StringComparison]::OrdinalIgnoreCase) -or
    -not $activeBeforeCleanup.Equals($OffFormatGuid, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Cleanup requires Spatial Sound Off. DEFAULT=$defaultBeforeCleanup ACTIVE=$activeBeforeCleanup"
}

$disableArgs = @{
    ControlPath = $control
    EndpointControlPath = $endpointControl
    EndpointStatePath = $EndpointStatePath
    ReceiptPath = $liveReceiptPath
}
& $disableScript @disableArgs

if ($selectionReceipt.InitialRuntimeKeyPresent -eq $true) {
    $null = Invoke-NativeCaptured -Path $control -Arguments @(
        'runtime-stage-disabled',
        $initialRuntimeEndpoint,
        $initialRuntimeDll
    ) -Label 'Restore pre-selection disabled runtime state'
}

$runtimeAfter = Invoke-NativeCaptured -Path $control -Arguments @('runtime-status') -Label 'Final runtime status'
$runtimeAfterText = @($runtimeAfter.output) -join [Environment]::NewLine
if ($selectionReceipt.InitialRuntimeKeyPresent -eq $true) {
    if ($runtimeAfterText -notmatch 'SPATIAL_RUNTIME_STATUS\s+KEY=1\s+ENABLED=0\s+READY=0') {
        throw 'Final runtime state did not restore the pre-selection disabled key.'
    }
    $runtimeEndpointAfter = Get-OutputValue -Result $runtimeAfter -Name 'SPATIAL_RUNTIME_ENDPOINT' -Label 'Final runtime status'
    $runtimeDllAfter = Get-OutputValue -Result $runtimeAfter -Name 'SPATIAL_RUNTIME_REALTIME_DLL' -Label 'Final runtime status'
    if ($runtimeEndpointAfter -ne $initialRuntimeEndpoint -or $runtimeDllAfter -ne $initialRuntimeDll) {
        throw 'Final runtime endpoint/DLL identity differs from the pre-selection state.'
    }
}
elseif ($runtimeAfterText -notmatch 'SPATIAL_RUNTIME_STATUS\s+KEY=0\s+ENABLED=0\s+READY=0') {
    throw 'Final runtime state did not restore pre-selection key absence.'
}

$registrationAfter = Invoke-NativeCaptured -Path $control -Arguments @('status') -Label 'Final provider registration' -AllowedExitCodes @(3)
Require-Pattern -Result $registrationAfter -Pattern 'SPATIAL_PROVIDER_STATUS\s+ENCODER=0\s+COM=0' -Label 'Final provider registration'

$selectionAfter = Invoke-NativeCaptured -Path $control -Arguments @('selection-status', $endpointId) -Label 'Final Windows spatial selection'
Require-Pattern -Result $selectionAfter -Pattern 'OMNIPHONY_DEFAULT\s+0' -Label 'Final Windows spatial selection'
Require-Pattern -Result $selectionAfter -Pattern 'OMNIPHONY_ACTIVE\s+0' -Label 'Final Windows spatial selection'
$defaultAfter = Get-OutputValue -Result $selectionAfter -Name 'DEFAULT_FORMAT' -Label 'Final Windows spatial selection'
$activeAfter = Get-OutputValue -Result $selectionAfter -Name 'ACTIVE_FORMAT' -Label 'Final Windows spatial selection'
if (-not $defaultAfter.Equals($OffFormatGuid, [System.StringComparison]::OrdinalIgnoreCase) -or
    -not $activeAfter.Equals($OffFormatGuid, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Final Windows spatial state is not the original Off baseline.'
}

$receiptRoot = Split-Path -Parent $CleanupReceiptPath
if ($receiptRoot) {
    New-Item -ItemType Directory -Force -Path $receiptRoot | Out-Null
}
$receipt = [ordered]@{
    SchemaVersion = 1
    TimestampUtc = [DateTime]::UtcNow.ToString('o')
    Pass = $true
    State = 'deselected-cleaned-baseline-restored'
    EndpointId = $endpointId
    EndpointName = $endpointName
    Generation = [string]$selectionReceipt.Generation
    PackageSha256 = [string]$selectionReceipt.PackageSha256
    SpatialSoundOffVerified = $true
    ProviderRegistered = $false
    OmniphonyDefault = $false
    OmniphonyActive = $false
    InitialRuntimeKeyPresent = [bool]$selectionReceipt.InitialRuntimeKeyPresent
    InitialRuntimeStateRestored = $true
}
$receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $CleanupReceiptPath -Encoding UTF8

Write-Host ''
Write-Host 'SPATIAL_PROVIDER_SELECTION_CLEANUP_OK 1'
Write-Host 'SPATIAL_PROVIDER_WINDOWS_OFF_VERIFIED 1'
Write-Host 'SPATIAL_PROVIDER_UNREGISTERED_VERIFIED 1'
Write-Host 'SPATIAL_PROVIDER_PRESELECTION_RUNTIME_RESTORED 1'
Write-Host "Receipt: $CleanupReceiptPath"
