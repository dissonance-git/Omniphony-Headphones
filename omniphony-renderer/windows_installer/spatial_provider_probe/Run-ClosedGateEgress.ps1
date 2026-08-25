[CmdletBinding()]
param(
    [string]$EndpointId,

    [ValidateRange(250, 5000)]
    [int]$DurationMs = 1500,

    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Release',

    [string]$RealtimeDll,

    [string]$ProbePath,

    [string]$ResultPath,

    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$probeRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$sourceRendererCandidate = [System.IO.Path]::GetFullPath((Join-Path $probeRoot '..\..'))
$sourceMode = Test-Path -LiteralPath (Join-Path $sourceRendererCandidate 'Cargo.toml') -PathType Leaf
$installedRoot = Split-Path -Parent $probeRoot
$endpointBackup = if ($env:ProgramData) {
    Join-Path $env:ProgramData 'Omniphony\endpoint-backup.json'
} else {
    'C:\ProgramData\Omniphony\endpoint-backup.json'
}

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command not found: $Name"
    }
}

function Read-Counter([string[]]$Lines, [string]$Name) {
    $line = $Lines | Where-Object { $_ -match "^$([regex]::Escape($Name))\s+" } | Select-Object -Last 1
    if (-not $line) {
        throw "Probe completed but did not emit counter: $Name"
    }
    return [uint64](($line -split '\s+')[-1])
}

if ([string]::IsNullOrWhiteSpace($EndpointId)) {
    if (-not (Test-Path -LiteralPath $endpointBackup -PathType Leaf)) {
        throw "Endpoint ID was not supplied and Omniphony endpoint state was not found: $endpointBackup"
    }
    $savedEndpoint = Get-Content -LiteralPath $endpointBackup -Raw | ConvertFrom-Json
    if (-not $savedEndpoint.EndpointId) {
        throw "Omniphony endpoint state does not contain EndpointId: $endpointBackup"
    }
    $EndpointId = [string]$savedEndpoint.EndpointId
    if ($savedEndpoint.EndpointName) {
        Write-Host "Using installed Omniphony endpoint: $($savedEndpoint.EndpointName)"
    } else {
        Write-Host 'Using endpoint ID saved by the Omniphony installer.'
    }
}

if ([string]::IsNullOrWhiteSpace($ProbePath)) {
    $installedProbe = Join-Path $probeRoot 'OmniphonySpatialClosedGateEgressProbe.exe'
    if (Test-Path -LiteralPath $installedProbe -PathType Leaf) {
        $ProbePath = $installedProbe
    } elseif ($sourceMode) {
        $sourceRepoRoot = (Resolve-Path (Join-Path $sourceRendererCandidate '..')).Path
        $sourceBuildRoot = Join-Path $sourceRepoRoot 'build\spatial-provider'
        $ProbePath = Join-Path $sourceBuildRoot "$Configuration\OmniphonySpatialClosedGateEgressProbe.exe"
    } else {
        throw "Closed-gate egress probe not found beside the runner: $installedProbe"
    }
}

if ([string]::IsNullOrWhiteSpace($RealtimeDll)) {
    $installedRealtime = Join-Path $installedRoot 'APO\omniphony_realtime.dll'
    if (Test-Path -LiteralPath $installedRealtime -PathType Leaf) {
        $RealtimeDll = $installedRealtime
    } elseif ($sourceMode) {
        $RealtimeDll = Join-Path $sourceRendererCandidate 'target\release\omniphony_realtime.dll'
    } else {
        throw "Installed realtime DLL not found: $installedRealtime"
    }
}

$shouldBuild = $sourceMode -and (-not $SkipBuild)
if ($shouldBuild) {
    Require-Command cmake
    Require-Command cargo

    $sourceRepoRoot = (Resolve-Path (Join-Path $sourceRendererCandidate '..')).Path
    $sourceBuildRoot = Join-Path $sourceRepoRoot 'build\spatial-provider'

    Write-Host 'Building realtime renderer DLL...'
    Push-Location $sourceRendererCandidate
    try {
        cargo build --release -p realtime_ffi
        if ($LASTEXITCODE -ne 0) { throw "realtime_ffi build failed: $LASTEXITCODE" }
    }
    finally {
        Pop-Location
    }

    Write-Host 'Configuring closed-gate spatial probe...'
    cmake -S $probeRoot -B $sourceBuildRoot -A x64
    if ($LASTEXITCODE -ne 0) { throw "CMake configure failed: $LASTEXITCODE" }

    Write-Host 'Building closed-gate egress probe...'
    cmake --build $sourceBuildRoot --config $Configuration --target OmniphonySpatialClosedGateEgressProbe
    if ($LASTEXITCODE -ne 0) { throw "Probe build failed: $LASTEXITCODE" }
}

if (-not (Test-Path -LiteralPath $RealtimeDll -PathType Leaf)) {
    throw "Realtime DLL not found: $RealtimeDll"
}
if (-not (Test-Path -LiteralPath $ProbePath -PathType Leaf)) {
    throw "Closed-gate egress probe not found: $ProbePath"
}

if ([string]::IsNullOrWhiteSpace($ResultPath)) {
    if ($env:ProgramData) {
        $ResultPath = Join-Path $env:ProgramData 'Omniphony\spatial-closed-gate-egress-last.json'
    } else {
        $ResultPath = Join-Path $probeRoot 'spatial-closed-gate-egress-last.json'
    }
}

Write-Warning 'This is a short audible low-level physical-endpoint diagnostic.'
Write-Host 'Safety boundary: no Spatial Sound provider registration, selection, or public provider gate activation is performed.'
Write-Host "Endpoint: $EndpointId"
Write-Host "Duration: $DurationMs ms"
Write-Host "Probe: $ProbePath"
Write-Host "Realtime DLL: $RealtimeDll"

$output = @(& $ProbePath $RealtimeDll $EndpointId $DurationMs 2>&1 | ForEach-Object { "$_" })
$exitCode = $LASTEXITCODE
$output | ForEach-Object { Write-Host $_ }

if ($exitCode -ne 0) {
    throw "Closed-gate egress probe failed with exit code $exitCode"
}

$required = @(
    'SPATIAL_CLOSED_GATE_EGRESS_OK 1',
    'SPATIAL_CLOSED_GATE_EGRESS_COM_TO_CURRENT 1',
    'SPATIAL_CLOSED_GATE_EGRESS_CURRENT_TO_QUEUE 1',
    'SPATIAL_CLOSED_GATE_EGRESS_ENDPOINT_EVENT_CLOCK 1',
    'SPATIAL_CLOSED_GATE_EGRESS_RAW_RENDER_CLIENT 1',
    'SPATIAL_CLOSED_GATE_EGRESS_PROVIDER_REGISTERED 0',
    'SPATIAL_CLOSED_GATE_EGRESS_PROVIDER_SELECTED 0',
    'SPATIAL_CLOSED_GATE_EGRESS_PUBLIC_PROVIDER_GATE_OPENED 0'
)
foreach ($marker in $required) {
    if (-not ($output -contains $marker)) {
        throw "Missing required success/safety marker: $marker"
    }
}

$drainCycles = Read-Counter $output 'SPATIAL_CLOSED_GATE_EGRESS_DRAIN_CYCLES'
$realFrames = Read-Counter $output 'SPATIAL_CLOSED_GATE_EGRESS_REAL_FRAMES'
$silenceFrames = Read-Counter $output 'SPATIAL_CLOSED_GATE_EGRESS_SILENCE_FRAMES'
$droppedFrames = Read-Counter $output 'SPATIAL_CLOSED_GATE_EGRESS_QUEUE_DROPPED_FRAMES'
$underrunFrames = Read-Counter $output 'SPATIAL_CLOSED_GATE_EGRESS_QUEUE_UNDERRUN_FRAMES'

if ($drainCycles -eq 0) { throw 'The physical endpoint event clock never drained a render cycle.' }
if ($realFrames -eq 0) { throw 'No real rendered frames reached the endpoint pump.' }
if ($droppedFrames -ne 0) { throw "Producer dropped frames: $droppedFrames" }

$receipt = [ordered]@{
    SchemaVersion = 1
    TimestampUtc = [DateTime]::UtcNow.ToString('o')
    Pass = $true
    EndpointId = $EndpointId
    DurationMs = $DurationMs
    ProbePath = $ProbePath
    RealtimeDll = $RealtimeDll
    DrainCycles = $drainCycles
    RealFrames = $realFrames
    SilenceFrames = $silenceFrames
    ProducerDroppedFrames = $droppedFrames
    UnderrunFrames = $underrunFrames
    ProviderRegisteredByProbe = $false
    ProviderSelectedByProbe = $false
    PublicProviderGateOpened = $false
    RawOutput = $output
}

try {
    $resultDirectory = Split-Path -Parent $ResultPath
    if ($resultDirectory) {
        New-Item -ItemType Directory -Path $resultDirectory -Force | Out-Null
    }
    $receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ResultPath -Encoding UTF8
    Write-Host "Receipt: $ResultPath"
} catch {
    Write-Warning "Physical egress passed but the JSON receipt could not be written: $($_.Exception.Message)"
}

Write-Host ''
Write-Host 'Closed-gate physical egress PASS.'
Write-Host "Drain cycles: $drainCycles"
Write-Host "Real frames: $realFrames"
Write-Host "Producer drops: $droppedFrames"
Write-Host "Measured underrun frames: $underrunFrames"
