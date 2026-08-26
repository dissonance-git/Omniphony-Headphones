[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$StageManifestPath,

    [Parameter(Mandatory = $true)]
    [string]$PreflightReportPath,

    [string]$ReportPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-NativeCaptured {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)][string]$Label,
        [int[]]$AllowedExitCodes = @(0)
    )

    $previous = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $lines = @(& $Path @Arguments 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
        if ($null -eq $code) { $code = 0 }
    }
    finally {
        $ErrorActionPreference = $previous
    }

    $lines | ForEach-Object { Write-Host $_ }
    if ($AllowedExitCodes -notcontains [int]$code) {
        throw "$Label failed with exit code $code."
    }

    return [ordered]@{
        exit_code = [int]$code
        output = $lines
    }
}

function Require-Pattern {
    param(
        [Parameter(Mandatory = $true)][object]$Result,
        [Parameter(Mandatory = $true)][string]$Pattern,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $text = @($Result.output) -join [Environment]::NewLine
    if ($text -notmatch $Pattern) {
        throw "$Label did not satisfy required output pattern: $Pattern"
    }
}

function Test-PathWithin {
    param(
        [Parameter(Mandatory = $true)][string]$Child,
        [Parameter(Mandatory = $true)][string]$Parent
    )

    $childFull = [System.IO.Path]::GetFullPath($Child).TrimEnd('\')
    $parentFull = [System.IO.Path]::GetFullPath($Parent).TrimEnd('\')
    if ($childFull.Equals($parentFull, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $true
    }
    return $childFull.StartsWith($parentFull + '\', [System.StringComparison]::OrdinalIgnoreCase)
}

function Write-StateSnapshot {
    param(
        [Parameter(Mandatory = $true)][string]$CaptureScript,
        [Parameter(Mandatory = $true)][string]$ControlPath,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    $lines = @(& $CaptureScript -ControlPath $ControlPath 2>&1 | ForEach-Object { "$_" })
    $lines | Set-Content -LiteralPath $Destination -Encoding UTF8
}

if ([Environment]::Is64BitOperatingSystem -and -not [Environment]::Is64BitProcess) {
    throw 'Spatial-provider activation test must run from a 64-bit PowerShell process on 64-bit Windows.'
}

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Spatial-provider activation test requires an elevated Administrator terminal.'
}

if (-not (Test-Path -LiteralPath $StageManifestPath -PathType Leaf)) {
    throw "Stage manifest is missing: $StageManifestPath"
}
if (-not (Test-Path -LiteralPath $PreflightReportPath -PathType Leaf)) {
    throw "Preflight report is missing: $PreflightReportPath"
}

$stagePath = (Resolve-Path -LiteralPath $StageManifestPath).Path
$preflightPath = (Resolve-Path -LiteralPath $PreflightReportPath).Path
$stage = Get-Content -LiteralPath $stagePath -Raw | ConvertFrom-Json
$preflight = Get-Content -LiteralPath $preflightPath -Raw | ConvertFrom-Json

if ($stage.schema -ne 'omniphony.windows.spatial-provider-stage.v1' -or $stage.state -ne 'staged-not-registered') {
    throw 'Activation test requires a current inert spatial-provider stage manifest.'
}
if ($preflight.schema -ne 'omniphony.windows.spatial-provider-preflight.v1' -or $preflight.state -ne 'preflight-passed-output-initialized-no-provider-mutation') {
    throw 'Activation test requires a current successful inert preflight report.'
}
if ([string]$stage.generation -ne [string]$preflight.generation -or [string]$stage.package_sha256 -ne [string]$preflight.package_sha256) {
    throw 'Stage and preflight do not describe the same immutable provider generation.'
}
if ($preflight.live_runtime_gate_closed -ne $true -or $preflight.omniphony_not_selected_before_preflight -ne $true -or $preflight.registry_mutated -ne $false -or $preflight.provider_selected -ne $false) {
    throw 'Activation test refuses a preflight that did not prove a closed, unselected starting state.'
}

$generationRoot = [System.IO.Path]::GetFullPath([string]$stage.generation_root)
$providerDll = [System.IO.Path]::GetFullPath([string]$stage.provider_dll)
$realtimeDll = [System.IO.Path]::GetFullPath([string]$stage.realtime_dll)
$endpointId = [string]$preflight.physical_endpoint_id
$control = Join-Path $generationRoot 'OmniphonySpatialProbeCtl.exe'
$smoke = Join-Path $generationRoot 'OmniphonySpatialProbeSmoke.exe'
$capture = Join-Path $generationRoot 'CaptureSpatialProviderState.ps1'

foreach ($path in @($providerDll, $realtimeDll, $control, $smoke, $capture)) {
    if (-not (Test-PathWithin -Child $path -Parent $generationRoot)) {
        throw "Activation-test path escaped immutable generation: $path"
    }
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Activation-test file is missing: $path"
    }
}
if ([string]::IsNullOrWhiteSpace($endpointId)) {
    throw 'Preflight report does not identify a physical endpoint.'
}

$runtimeBefore = Invoke-NativeCaptured -Path $control -Arguments @('runtime-status') -Label 'Initial runtime status'
Require-Pattern -Result $runtimeBefore -Pattern 'SPATIAL_RUNTIME_STATUS\s+KEY=0\s+ENABLED=0\s+READY=0' -Label 'Initial runtime status'

$registrationBefore = Invoke-NativeCaptured -Path $control -Arguments @('status') -Label 'Initial registration status' -AllowedExitCodes @(3)
Require-Pattern -Result $registrationBefore -Pattern 'SPATIAL_PROVIDER_STATUS\s+ENCODER=0\s+COM=0' -Label 'Initial registration status'

$selectionBefore = Invoke-NativeCaptured -Path $control -Arguments @('selection-status', $endpointId) -Label 'Initial selection status'
Require-Pattern -Result $selectionBefore -Pattern 'OMNIPHONY_DEFAULT\s+0' -Label 'Initial selection status'
Require-Pattern -Result $selectionBefore -Pattern 'OMNIPHONY_ACTIVE\s+0' -Label 'Initial selection status'

if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = Join-Path ([System.IO.Path]::GetDirectoryName($preflightPath)) 'activation-generation.json'
}
$reportFullPath = [System.IO.Path]::GetFullPath($ReportPath)
$reportDirectory = [System.IO.Path]::GetDirectoryName($reportFullPath)
New-Item -ItemType Directory -Force -Path $reportDirectory | Out-Null

$beforeSnapshot = Join-Path $reportDirectory 'activation-before.txt'
$afterSnapshot = Join-Path $reportDirectory 'activation-after.txt'
Write-StateSnapshot -CaptureScript $capture -ControlPath $control -Destination $beforeSnapshot

$activationPassed = $false
$cleanupPassed = $false
try {
    Invoke-NativeCaptured -Path $control -Arguments @('register', $providerDll) -Label 'Register immutable provider generation' | Out-Null
    Invoke-NativeCaptured -Path $control -Arguments @('runtime-enable', $endpointId, $realtimeDll) -Label 'Enable exact provider runtime gate' | Out-Null

    $runtimeEnabled = Invoke-NativeCaptured -Path $control -Arguments @('runtime-status') -Label 'Enabled runtime status'
    Require-Pattern -Result $runtimeEnabled -Pattern 'SPATIAL_RUNTIME_STATUS\s+KEY=1\s+ENABLED=1\s+READY=1' -Label 'Enabled runtime status'

    $runtimeSmoke = Invoke-NativeCaptured -Path $smoke -Arguments @($providerDll, '--expect-runtime') -Label 'Gated public stream activation smoke'
    Require-Pattern -Result $runtimeSmoke -Pattern 'SPATIAL_PROVIDER_STREAM_AVAILABLE\s+1' -Label 'Gated public stream activation smoke'
    Require-Pattern -Result $runtimeSmoke -Pattern 'SPATIAL_PROVIDER_RUNTIME_ACTIVATION_OK\s+1' -Label 'Gated public stream activation smoke'
    Require-Pattern -Result $runtimeSmoke -Pattern 'SPATIAL_PROVIDER_RUNTIME_STREAM_STARTED\s+0' -Label 'Gated public stream activation smoke'

    $selectionDuring = Invoke-NativeCaptured -Path $control -Arguments @('selection-status', $endpointId) -Label 'Selection status during activation test'
    Require-Pattern -Result $selectionDuring -Pattern 'OMNIPHONY_DEFAULT\s+0' -Label 'Selection status during activation test'
    Require-Pattern -Result $selectionDuring -Pattern 'OMNIPHONY_ACTIVE\s+0' -Label 'Selection status during activation test'

    $activationPassed = $true
}
finally {
    try {
        Invoke-NativeCaptured -Path $control -Arguments @('runtime-disable') -Label 'Disable runtime gate after activation test' | Out-Null
    }
    catch {
        Write-Warning $_
    }
    try {
        Invoke-NativeCaptured -Path $control -Arguments @('unregister') -Label 'Unregister provider after activation test' | Out-Null
    }
    catch {
        Write-Warning $_
    }

    $runtimeAfter = Invoke-NativeCaptured -Path $control -Arguments @('runtime-status') -Label 'Final runtime status'
    $registrationAfter = Invoke-NativeCaptured -Path $control -Arguments @('status') -Label 'Final registration status' -AllowedExitCodes @(3)
    $selectionAfter = Invoke-NativeCaptured -Path $control -Arguments @('selection-status', $endpointId) -Label 'Final selection status'

    $runtimeAfterText = @($runtimeAfter.output) -join [Environment]::NewLine
    $registrationAfterText = @($registrationAfter.output) -join [Environment]::NewLine
    $selectionAfterText = @($selectionAfter.output) -join [Environment]::NewLine
    $cleanupPassed =
        $runtimeAfterText -match 'SPATIAL_RUNTIME_STATUS\s+KEY=0\s+ENABLED=0\s+READY=0' -and
        $registrationAfterText -match 'SPATIAL_PROVIDER_STATUS\s+ENCODER=0\s+COM=0' -and
        $selectionAfterText -match 'OMNIPHONY_DEFAULT\s+0' -and
        $selectionAfterText -match 'OMNIPHONY_ACTIVE\s+0'

    Write-StateSnapshot -CaptureScript $capture -ControlPath $control -Destination $afterSnapshot
}

if (-not $activationPassed) {
    throw 'Gated public provider activation did not complete successfully.'
}
if (-not $cleanupPassed) {
    throw 'Gated public provider activation succeeded but owned-state rollback did not verify cleanly.'
}

$report = [ordered]@{
    schema = 'omniphony.windows.spatial-provider-activation-test.v1'
    state = 'activation-proven-rolled-back-unselected'
    generation = [string]$stage.generation
    package_sha256 = [string]$stage.package_sha256
    stage_manifest = $stagePath
    preflight_report = $preflightPath
    physical_endpoint_id = $endpointId
    provider_registered_temporarily = $true
    runtime_gate_enabled_temporarily = $true
    public_stream_available = $true
    public_stream_activated = $true
    public_stream_started = $false
    provider_selection_changed = $false
    rollback_verified = $true
    before_snapshot = $beforeSnapshot
    after_snapshot = $afterSnapshot
    activation_test_utc = [DateTime]::UtcNow.ToString('o')
}

$tempReport = "$reportFullPath.tmp-$PID"
try {
    $report | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $tempReport -Encoding UTF8
    Move-Item -LiteralPath $tempReport -Destination $reportFullPath -Force
}
finally {
    if (Test-Path -LiteralPath $tempReport -PathType Leaf) {
        Remove-Item -LiteralPath $tempReport -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "SPATIAL_PROVIDER_ACTIVATION_TEST_OK GENERATION=$($stage.generation)"
Write-Host "SPATIAL_PROVIDER_ACTIVATION_TEST_REPORT $reportFullPath"
Write-Host 'SPATIAL_PROVIDER_PUBLIC_STREAM_AVAILABLE 1'
Write-Host 'SPATIAL_PROVIDER_PUBLIC_STREAM_ACTIVATED 1'
Write-Host 'SPATIAL_PROVIDER_PUBLIC_STREAM_STARTED 0'
Write-Host 'SPATIAL_PROVIDER_SELECTION_CHANGED 0'
Write-Host 'SPATIAL_PROVIDER_ACTIVATION_ROLLBACK_VERIFIED 1'
