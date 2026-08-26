param(
    [Parameter(Mandatory = $true)]
    [string]$StageManifestPath,

    [Parameter(Mandatory = $true)]
    [string]$PhysicalEndpointId,

    [string]$ReportPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Get-Sha256 {
    param([Parameter(Mandatory = $true)][string]$Path)
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
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

function Invoke-NativeCaptured {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)][string]$Label
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Label executable is missing: $Path"
    }

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
    if ($code -ne 0) {
        throw "$Label failed with exit code $code."
    }

    return [ordered]@{
        exit_code = $code
        output = $lines
    }
}

function Assert-Marker {
    param(
        [Parameter(Mandatory = $true)][object]$Result,
        [Parameter(Mandatory = $true)][string]$Marker,
        [Parameter(Mandatory = $true)][string]$Label
    )

    if (-not (@($Result.output) -contains $Marker)) {
        throw "$Label did not emit required marker: $Marker"
    }
}

function Get-UInt32Marker {
    param(
        [Parameter(Mandatory = $true)][object]$Result,
        [Parameter(Mandatory = $true)][string]$Prefix,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $line = @($Result.output | Where-Object { $_.StartsWith($Prefix + ' ') } | Select-Object -First 1)
    if ($line.Count -ne 1) {
        throw "$Label did not emit exactly one numeric marker: $Prefix"
    }
    $text = $line[0].Substring($Prefix.Length).Trim()
    [uint32]$value = 0
    if (-not [uint32]::TryParse($text, [ref]$value)) {
        throw "$Label emitted an invalid numeric marker: $($line[0])"
    }
    return $value
}

if ([Environment]::Is64BitOperatingSystem -and -not [Environment]::Is64BitProcess) {
    throw 'Spatial-provider activation preflight must run from a 64-bit PowerShell process on 64-bit Windows.'
}

if ([string]::IsNullOrWhiteSpace($PhysicalEndpointId)) {
    throw 'PhysicalEndpointId must identify the exact physical render endpoint intended to receive Omniphony stereo output.'
}

if (-not (Test-Path -LiteralPath $StageManifestPath -PathType Leaf)) {
    throw "Spatial-provider stage manifest is missing: $StageManifestPath"
}

$manifestPathResolved = (Resolve-Path -LiteralPath $StageManifestPath).Path
$manifest = Get-Content -LiteralPath $manifestPathResolved -Raw | ConvertFrom-Json

if ($manifest.schema -ne 'omniphony.windows.spatial-provider-stage.v1') {
    throw "Unsupported spatial-provider stage manifest schema: $($manifest.schema)"
}
if ($manifest.state -ne 'staged-not-registered') {
    throw "Spatial-provider stage is not inert: state=$($manifest.state)"
}
if ($manifest.registry_mutated -ne $false -or $manifest.provider_selected -ne $false) {
    throw 'Spatial-provider preflight refuses a stage manifest that records registration or selection mutation.'
}
if ($manifest.exact_file_set_verified -ne $true -or $manifest.final_path_smokes_verified -ne $true) {
    throw 'Spatial-provider stage manifest does not record final immutable verification.'
}
if ($manifest.dynamic_object_contract_verified -ne $true -or
    $manifest.spatial_object_abi_reset_verified -ne $true -or
    $manifest.composed_dynamic_render_path_verified -ne $true) {
    throw 'Spatial-provider stage predates the dynamic object/reset contract; restage the current package before activation preflight.'
}
if ($manifest.clock_domain_queue_verified -ne $true) {
    throw 'Spatial-provider stage predates the clock-domain queue contract; restage the current package before activation preflight.'
}
if (-not $manifest.app_root -or -not $manifest.generation_root) {
    throw 'Spatial-provider stage manifest is missing app_root or generation_root.'
}

$appRoot = [System.IO.Path]::GetFullPath([string]$manifest.app_root)
$generationRoot = [System.IO.Path]::GetFullPath([string]$manifest.generation_root)
if (-not (Test-PathWithin -Child $generationRoot -Parent (Join-Path $appRoot 'SpatialProvider\generations'))) {
    throw "Spatial-provider generation is outside the managed generations root: $generationRoot"
}
if (-not (Test-Path -LiteralPath $generationRoot -PathType Container)) {
    throw "Spatial-provider staged generation directory is missing: $generationRoot"
}

$expected = [ordered]@{}
foreach ($property in $manifest.file_sha256.PSObject.Properties) {
    $expected[$property.Name] = ([string]$property.Value).ToLowerInvariant()
}
if ($expected.Count -eq 0) {
    throw 'Spatial-provider stage manifest contains no file hashes.'
}

$actualItems = @(Get-ChildItem -LiteralPath $generationRoot -Force)
$directories = @($actualItems | Where-Object { $_.PSIsContainer })
if ($directories.Count -ne 0) {
    $names = ($directories | ForEach-Object { $_.Name }) -join ', '
    throw "Spatial-provider staged generation contains unexpected directories: $names"
}

$actualNames = @($actualItems | ForEach-Object { $_.Name } | Sort-Object)
$expectedNames = @($expected.Keys | Sort-Object)
$diff = @(Compare-Object -ReferenceObject $expectedNames -DifferenceObject $actualNames)
if ($diff.Count -ne 0) {
    $detail = ($diff | ForEach-Object { "$($_.SideIndicator)$($_.InputObject)" }) -join ', '
    throw "Spatial-provider staged generation file set changed after staging: [$detail]"
}

foreach ($name in $expected.Keys) {
    $path = Join-Path $generationRoot $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Spatial-provider staged file is missing: $path"
    }
    $actualHash = Get-Sha256 $path
    if ($actualHash -ne $expected[$name]) {
        throw "Spatial-provider staged file hash changed: $path expected=$($expected[$name]) actual=$actualHash"
    }
}

foreach ($requiredField in @('provider_dll', 'realtime_dll', 'object_stream_smoke', 'object_realtime_smoke', 'stereo_queue_smoke', 'raw_output_probe', 'raw_output_sink_probe')) {
    if (-not $manifest.$requiredField) {
        throw "Spatial-provider stage manifest is missing required current field: $requiredField"
    }
}

$providerDll = [System.IO.Path]::GetFullPath([string]$manifest.provider_dll)
$realtimeDll = [System.IO.Path]::GetFullPath([string]$manifest.realtime_dll)
$providerCtl = Join-Path $generationRoot 'OmniphonySpatialProbeCtl.exe'
$objectStreamSmoke = [System.IO.Path]::GetFullPath([string]$manifest.object_stream_smoke)
$objectRealtimeSmoke = [System.IO.Path]::GetFullPath([string]$manifest.object_realtime_smoke)
$stereoQueueSmoke = [System.IO.Path]::GetFullPath([string]$manifest.stereo_queue_smoke)
$rawOutputProbe = [System.IO.Path]::GetFullPath([string]$manifest.raw_output_probe)
$rawOutputSinkProbe = [System.IO.Path]::GetFullPath([string]$manifest.raw_output_sink_probe)
foreach ($ownedPath in @($providerDll, $realtimeDll, $providerCtl, $objectStreamSmoke, $objectRealtimeSmoke, $stereoQueueSmoke, $rawOutputProbe, $rawOutputSinkProbe)) {
    if (-not (Test-PathWithin -Child $ownedPath -Parent $generationRoot)) {
        throw "Spatial-provider manifest points outside its immutable generation: $ownedPath"
    }
}

$runtimeStatus = Invoke-NativeCaptured `
    -Path $providerCtl `
    -Arguments @('runtime-status') `
    -Label 'Live provider runtime-gate status'
$runtimeStatusText = @($runtimeStatus.output) -join "`n"
if ($runtimeStatusText -notmatch 'SPATIAL_RUNTIME_STATUS\s+KEY=(?:0|1)\s+ENABLED=0\s+READY=0') {
    throw 'Activation preflight requires the live Omniphony provider runtime gate to be closed.'
}

$selectionStatus = Invoke-NativeCaptured `
    -Path $providerCtl `
    -Arguments @('selection-status', $PhysicalEndpointId) `
    -Label 'Live spatial provider selection status'
$selectionText = @($selectionStatus.output) -join "`n"
if ($selectionText -notmatch 'OMNIPHONY_DEFAULT\s+0' -or
    $selectionText -notmatch 'OMNIPHONY_ACTIVE\s+0') {
    throw 'Activation preflight requires Omniphony to be neither default nor active on the physical endpoint.'
}

$capability = Invoke-NativeCaptured `
    -Path (Join-Path $generationRoot 'OmniphonySpatialProbeSmoke.exe') `
    -Arguments @($providerDll) `
    -Label 'Final-path provider capability smoke'

$staticStream = Invoke-NativeCaptured `
    -Path (Join-Path $generationRoot 'OmniphonySpatialStaticStreamSmoke.exe') `
    -Label 'Final-path static stream lifecycle smoke'

$objectStream = Invoke-NativeCaptured `
    -Path $objectStreamSmoke `
    -Label 'Final-path dynamic object lifecycle smoke'
Assert-Marker -Result $objectStream -Marker 'SPATIAL_OBJECT_STREAM_STABLE_ID_OK 1' -Label 'Final-path dynamic object lifecycle smoke'
Assert-Marker -Result $objectStream -Marker 'SPATIAL_OBJECT_STREAM_XYZ_PERSISTENCE_OK 1' -Label 'Final-path dynamic object lifecycle smoke'
Assert-Marker -Result $objectStream -Marker 'SPATIAL_OBJECT_STREAM_RESET_PROPAGATION_OK 1' -Label 'Final-path dynamic object lifecycle smoke'

$objectRealtime = Invoke-NativeCaptured `
    -Path $objectRealtimeSmoke `
    -Arguments @($realtimeDll) `
    -Label 'Final-path dynamic object realtime ABI smoke'
$objectRealtimeText = @($objectRealtime.output) -join "`n"
if ($objectRealtimeText -notmatch 'SPATIAL_OBJECT_REALTIME_ABI_OK\s+ABI=0\.7\b.*\bRESET=1') {
    throw 'Final-path dynamic object realtime ABI/reset marker missing.'
}

$realtimeBridge = Invoke-NativeCaptured `
    -Path (Join-Path $generationRoot 'OmniphonySpatialRealtimeBridgeSmoke.exe') `
    -Arguments @($realtimeDll) `
    -Label 'Final-path realtime bridge smoke'
Assert-Marker -Result $realtimeBridge -Marker 'SPATIAL_COM_TO_CURRENT_OK 1' -Label 'Final-path realtime bridge smoke'
Assert-Marker -Result $realtimeBridge -Marker 'SPATIAL_COM_TO_STEREO_QUEUE_OK 1' -Label 'Final-path realtime bridge smoke'
Assert-Marker -Result $realtimeBridge -Marker 'SPATIAL_DYNAMIC_COM_TO_CURRENT_OK 1' -Label 'Final-path realtime bridge smoke'
Assert-Marker -Result $realtimeBridge -Marker 'SPATIAL_DYNAMIC_COM_TO_STEREO_QUEUE_OK 1' -Label 'Final-path realtime bridge smoke'
Assert-Marker -Result $realtimeBridge -Marker 'SPATIAL_FINAL_ENDPOINT_PROVEN 0' -Label 'Final-path realtime bridge smoke'

$queueSmoke = Invoke-NativeCaptured `
    -Path $stereoQueueSmoke `
    -Label 'Stereo clock-domain queue smoke'
Assert-Marker -Result $queueSmoke -Marker 'SPATIAL_STEREO_QUEUE_OK 1' -Label 'Stereo clock-domain queue smoke'
Assert-Marker -Result $queueSmoke -Marker 'SPATIAL_STEREO_QUEUE_VARIABLE_CONSUMER_PERIODS 1' -Label 'Stereo clock-domain queue smoke'

$rawOutput = Invoke-NativeCaptured `
    -Path $rawOutputProbe `
    -Arguments @($PhysicalEndpointId) `
    -Label 'Physical endpoint RAW output capability preflight'
Assert-Marker -Result $rawOutput -Marker 'SPATIAL_RAW_OUTPUT_PROBE_OK 1' -Label 'Physical endpoint RAW output capability preflight'
Assert-Marker -Result $rawOutput -Marker 'SPATIAL_RAW_OUTPUT_STREAM_INITIALIZED 0' -Label 'Physical endpoint RAW output capability preflight'
Assert-Marker -Result $rawOutput -Marker 'SPATIAL_RAW_OUTPUT_STREAM_STARTED 0' -Label 'Physical endpoint RAW output capability preflight'

$desiredSupported = @($rawOutput.output) -contains 'SPATIAL_RAW_OUTPUT_DESIRED_SUPPORTED 1'
$periodQueryOk = @($rawOutput.output) -contains 'SPATIAL_RAW_OUTPUT_PERIOD_QUERY_OK 1'
$period480Legal = @($rawOutput.output) -contains 'SPATIAL_RAW_OUTPUT_480_PERIOD_LEGAL 1'
if (-not $desiredSupported) {
    throw 'Physical endpoint does not report support for the staged stereo float32 / 48 kHz output contract.'
}
if (-not $periodQueryOk) {
    throw 'Physical endpoint did not provide the shared-engine period constraints needed for safe output planning.'
}

# Initialize, but deliberately do not start, the exact endpoint render stream.
# The sink chooses the endpoint-reported default legal period. Omniphony keeps
# its 480-frame render quantum on the producer side and adapts cadence through
# the preallocated SPSC queue instead of rejecting a valid device period.
$rawOutputSink = Invoke-NativeCaptured `
    -Path $rawOutputSinkProbe `
    -Arguments @($PhysicalEndpointId) `
    -Label 'Physical endpoint RAW output lifecycle preflight'
Assert-Marker -Result $rawOutputSink -Marker 'SPATIAL_RAW_OUTPUT_SINK_OK 1' -Label 'Physical endpoint RAW output lifecycle preflight'
Assert-Marker -Result $rawOutputSink -Marker 'SPATIAL_RAW_OUTPUT_SINK_INITIALIZED 1' -Label 'Physical endpoint RAW output lifecycle preflight'
Assert-Marker -Result $rawOutputSink -Marker 'SPATIAL_RAW_OUTPUT_SINK_STARTED 0' -Label 'Physical endpoint RAW output lifecycle preflight'
Assert-Marker -Result $rawOutputSink -Marker 'SPATIAL_RAW_OUTPUT_SINK_RENDER_CLIENT 1' -Label 'Physical endpoint RAW output lifecycle preflight'
Assert-Marker -Result $rawOutputSink -Marker 'SPATIAL_RAW_OUTPUT_SINK_EVENT_HANDLE 1' -Label 'Physical endpoint RAW output lifecycle preflight'
$endpointPeriodFrames = Get-UInt32Marker `
    -Result $rawOutputSink `
    -Prefix 'SPATIAL_RAW_OUTPUT_SINK_PERIOD_FRAMES' `
    -Label 'Physical endpoint RAW output lifecycle preflight'
$clockDomainAdapterRequired = $endpointPeriodFrames -ne 480

$report = [ordered]@{
    schema = 'omniphony.windows.spatial-provider-preflight.v1'
    state = 'preflight-passed-output-initialized-no-provider-mutation'
    generation = [string]$manifest.generation
    package_sha256 = [string]$manifest.package_sha256
    stage_manifest = $manifestPathResolved
    generation_root = $generationRoot
    physical_endpoint_id = $PhysicalEndpointId
    os_64_bit = [Environment]::Is64BitOperatingSystem
    process_64_bit = [Environment]::Is64BitProcess
    exact_file_set_verified = $true
    all_file_hashes_verified = $true
    live_runtime_gate_closed = $true
    omniphony_not_selected_before_preflight = $true
    final_path_capability_smoke_verified = $true
    final_path_static_stream_smoke_verified = $true
    final_path_dynamic_object_smoke_verified = $true
    final_path_dynamic_object_abi_reset_verified = $true
    final_path_realtime_bridge_smoke_verified = $true
    dynamic_com_to_current_verified_registry_free = $true
    dynamic_current_stereo_to_queue_verified_registry_free = $true
    com_to_current_verified_registry_free = $true
    current_stereo_to_queue_verified_registry_free = $true
    clock_domain_queue_verified = $true
    renderer_quantum_frames = 480
    desired_stereo_output_supported = $true
    output_period_query_verified = $true
    renderer_480_period_directly_legal = $period480Legal
    endpoint_period_frames = $endpointPeriodFrames
    clock_domain_adapter_required = $clockDomainAdapterRequired
    output_stream_initialized = $true
    output_stream_started = $false
    registry_mutated = $false
    provider_selected = $false
    preflight_utc = [DateTime]::UtcNow.ToString('o')
}

if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = Join-Path ([System.IO.Path]::GetDirectoryName($manifestPathResolved)) 'preflight-generation.json'
}
$reportFullPath = [System.IO.Path]::GetFullPath($ReportPath)
$reportDirectory = [System.IO.Path]::GetDirectoryName($reportFullPath)
if (-not (Test-Path -LiteralPath $reportDirectory -PathType Container)) {
    New-Item -ItemType Directory -Force -Path $reportDirectory | Out-Null
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

Write-Host "SPATIAL_PROVIDER_PREFLIGHT_OK GENERATION=$($manifest.generation)"
Write-Host "SPATIAL_PROVIDER_PREFLIGHT_ENDPOINT $PhysicalEndpointId"
Write-Host "SPATIAL_PROVIDER_PREFLIGHT_REPORT $reportFullPath"
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_LIVE_RUNTIME_GATE_CLOSED 1'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_OMNIPHONY_NOT_SELECTED 1'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_COM_TO_CURRENT 1'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_DYNAMIC_OBJECT_CONTRACT 1'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_OBJECT_ABI_RESET 1'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_DYNAMIC_COM_TO_CURRENT 1'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_DYNAMIC_CURRENT_TO_STEREO_QUEUE 1'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_CURRENT_TO_STEREO_QUEUE 1'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_CLOCK_DOMAIN_QUEUE 1'
Write-Host "SPATIAL_PROVIDER_PREFLIGHT_RENDER_QUANTUM_FRAMES 480"
Write-Host "SPATIAL_PROVIDER_PREFLIGHT_ENDPOINT_PERIOD_FRAMES $endpointPeriodFrames"
Write-Host "SPATIAL_PROVIDER_PREFLIGHT_CLOCK_ADAPTER_REQUIRED $([int]$clockDomainAdapterRequired)"
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_OUTPUT_CONTRACT 1'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_OUTPUT_STREAM_INITIALIZED 1'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_OUTPUT_STREAM_STARTED 0'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_REGISTRY_MUTATED 0'
Write-Host 'SPATIAL_PROVIDER_PREFLIGHT_PROVIDER_SELECTED 0'
