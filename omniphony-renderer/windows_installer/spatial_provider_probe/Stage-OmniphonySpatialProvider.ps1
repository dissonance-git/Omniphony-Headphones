param(
    [Parameter(Mandatory = $true)]
    [string]$PackageRoot,

    [string]$AppRoot = (Join-Path $env:ProgramFiles 'Omniphony')
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-NativeChecked {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)][string]$Label
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
    if ($code -ne 0) {
        throw "$Label failed with exit code $code."
    }
}

function Get-RequiredFile {
    param([string]$Root, [string]$Name)
    $path = Join-Path $Root $Name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Missing spatial-provider package file: $path"
    }
    return (Resolve-Path -LiteralPath $path).Path
}

function Get-Sha256 {
    param([string]$Path)
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Assert-Hash {
    param([string]$Path, [string]$Expected)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Spatial-provider immutable generation is incomplete: $Path"
    }
    $actual = Get-Sha256 $Path
    if ($actual -ne $Expected) {
        throw "Spatial-provider staged file hash mismatch: $Path expected=$Expected actual=$actual"
    }
}

function Assert-ExactFileSet {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string[]]$ExpectedNames
    )

    if (-not (Test-Path -LiteralPath $Root -PathType Container)) {
        throw "Spatial-provider generation directory is missing: $Root"
    }

    $expected = @($ExpectedNames | Sort-Object)
    $actualItems = @(Get-ChildItem -LiteralPath $Root -Force)
    $directories = @($actualItems | Where-Object { $_.PSIsContainer })
    if ($directories.Count -ne 0) {
        $names = ($directories | ForEach-Object { $_.Name }) -join ', '
        throw "Spatial-provider immutable generation contains unexpected directories: $names"
    }

    $actual = @($actualItems | ForEach-Object { $_.Name } | Sort-Object)
    $diff = @(Compare-Object -ReferenceObject $expected -DifferenceObject $actual)
    if ($diff.Count -ne 0) {
        $detail = ($diff | ForEach-Object { "$($_.SideIndicator)$($_.InputObject)" }) -join ', '
        throw "Spatial-provider immutable generation file set mismatch: $Root [$detail]"
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

if ([Environment]::Is64BitOperatingSystem -and -not [Environment]::Is64BitProcess) {
    throw 'Spatial-provider staging must run from a 64-bit PowerShell process on 64-bit Windows to avoid Program Files and future registry-view redirection.'
}

if (-not (Test-Path -LiteralPath $PackageRoot -PathType Container)) {
    throw "Spatial-provider package root is missing or is not a directory: $PackageRoot"
}

$packageRootResolved = (Resolve-Path -LiteralPath $PackageRoot).Path
$AppRoot = [System.IO.Path]::GetFullPath($AppRoot)

$files = @(
    @{ Source = (Get-RequiredFile $packageRootResolved 'OmniphonySpatialProbe.dll'); Name = 'OmniphonySpatialProbe.dll' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'omniphony_realtime.dll'); Name = 'omniphony_realtime.dll' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'OmniphonySpatialProbeCtl.exe'); Name = 'OmniphonySpatialProbeCtl.exe' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'OmniphonySpatialProbeSmoke.exe'); Name = 'OmniphonySpatialProbeSmoke.exe' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'OmniphonySpatialStaticStreamSmoke.exe'); Name = 'OmniphonySpatialStaticStreamSmoke.exe' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'OmniphonySpatialObjectStreamSmoke.exe'); Name = 'OmniphonySpatialObjectStreamSmoke.exe' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'OmniphonySpatialObjectRealtimeSmoke.exe'); Name = 'OmniphonySpatialObjectRealtimeSmoke.exe' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'OmniphonySpatialRealtimeBridgeSmoke.exe'); Name = 'OmniphonySpatialRealtimeBridgeSmoke.exe' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'OmniphonySpatialStereoQueueSmoke.exe'); Name = 'OmniphonySpatialStereoQueueSmoke.exe' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'OmniphonySpatialRawOutputProbe.exe'); Name = 'OmniphonySpatialRawOutputProbe.exe' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'OmniphonySpatialRawOutputSinkProbe.exe'); Name = 'OmniphonySpatialRawOutputSinkProbe.exe' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'CaptureSpatialProviderState.ps1'); Name = 'CaptureSpatialProviderState.ps1' },
    @{ Source = (Get-RequiredFile $packageRootResolved 'Test-OmniphonySpatialProviderActivation.ps1'); Name = 'Test-OmniphonySpatialProviderActivation.ps1' }
)

foreach ($file in $files) {
    $file.Hash = Get-Sha256 $file.Source
}

$identity = ($files | Sort-Object Name | ForEach-Object { "$($_.Name)=$($_.Hash)" }) -join "`n"
$identityBytes = [System.Text.Encoding]::UTF8.GetBytes($identity)
$sha256 = [System.Security.Cryptography.SHA256]::Create()
try {
    $packageDigestBytes = $sha256.ComputeHash($identityBytes)
}
finally {
    $sha256.Dispose()
}
$packageDigest = [System.BitConverter]::ToString($packageDigestBytes).Replace('-', '').ToLowerInvariant()
$generation = $packageDigest.Substring(0, 24)

$providerEntry = $files | Where-Object { $_.Name -eq 'OmniphonySpatialProbe.dll' } | Select-Object -First 1
$runtimeEntry = $files | Where-Object { $_.Name -eq 'omniphony_realtime.dll' } | Select-Object -First 1
$providerHash = $providerEntry.Hash
$runtimeHash = $runtimeEntry.Hash
$expectedNames = @($files | ForEach-Object { $_.Name })

$spatialRoot = Join-Path $AppRoot 'SpatialProvider'
$generationsRoot = Join-Path $spatialRoot 'generations'
$generationRoot = Join-Path $generationsRoot $generation
$stagingRoot = Join-Path $generationsRoot ('.{0}.staging-{1}' -f $generation, $PID)
$manifestPath = Join-Path $spatialRoot 'staged-generation.json'

if (Test-PathWithin -Child $packageRootResolved -Parent $spatialRoot) {
    throw "Spatial-provider package root must not be inside the managed SpatialProvider tree: $packageRootResolved"
}
if (Test-PathWithin -Child $generationRoot -Parent $packageRootResolved) {
    throw "Spatial-provider managed generation must not be created inside the source package root: $generationRoot"
}

New-Item -ItemType Directory -Force -Path $generationsRoot | Out-Null

if (Test-Path -LiteralPath $generationRoot -PathType Container) {
    Assert-ExactFileSet -Root $generationRoot -ExpectedNames $expectedNames
    foreach ($file in $files) {
        Assert-Hash (Join-Path $generationRoot $file.Name) $file.Hash
    }
    Write-Host "SPATIAL_PROVIDER_GENERATION_REUSED $generationRoot"
}
else {
    if (Test-Path -LiteralPath $stagingRoot) {
        Remove-Item -LiteralPath $stagingRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $stagingRoot | Out-Null

    try {
        foreach ($file in $files) {
            Copy-Item -LiteralPath $file.Source -Destination (Join-Path $stagingRoot $file.Name) -Force
        }
        Assert-ExactFileSet -Root $stagingRoot -ExpectedNames $expectedNames
        foreach ($file in $files) {
            Assert-Hash (Join-Path $stagingRoot $file.Name) $file.Hash
        }

        Invoke-NativeChecked -Path (Join-Path $stagingRoot 'OmniphonySpatialProbeSmoke.exe') -Arguments @((Join-Path $stagingRoot 'OmniphonySpatialProbe.dll')) -Label 'Spatial provider capability smoke'
        Invoke-NativeChecked -Path (Join-Path $stagingRoot 'OmniphonySpatialStaticStreamSmoke.exe') -Label 'Spatial static stream lifecycle smoke'
        Invoke-NativeChecked -Path (Join-Path $stagingRoot 'OmniphonySpatialObjectStreamSmoke.exe') -Label 'Spatial dynamic object lifecycle smoke'
        Invoke-NativeChecked -Path (Join-Path $stagingRoot 'OmniphonySpatialObjectRealtimeSmoke.exe') -Arguments @((Join-Path $stagingRoot 'omniphony_realtime.dll')) -Label 'Spatial dynamic object ABI 0.7 smoke'
        Invoke-NativeChecked -Path (Join-Path $stagingRoot 'OmniphonySpatialRealtimeBridgeSmoke.exe') -Arguments @((Join-Path $stagingRoot 'omniphony_realtime.dll')) -Label 'Spatial composed static/dynamic realtime bridge smoke'
        Invoke-NativeChecked -Path (Join-Path $stagingRoot 'OmniphonySpatialStereoQueueSmoke.exe') -Label 'Spatial stereo clock-domain queue smoke'

        Move-Item -LiteralPath $stagingRoot -Destination $generationRoot
        Write-Host "SPATIAL_PROVIDER_GENERATION_STAGED $generationRoot"
    }
    catch {
        if (Test-Path -LiteralPath $stagingRoot) {
            Remove-Item -LiteralPath $stagingRoot -Recurse -Force -ErrorAction SilentlyContinue
        }
        throw
    }
}

# Re-verify the immutable directory after promotion/reuse and re-run all
# package-coupling smokes from the final path. This catches extra-file drift,
# hash drift, and path-sensitive loading mistakes before a later transaction is
# allowed to point Windows at this generation.
Assert-ExactFileSet -Root $generationRoot -ExpectedNames $expectedNames
foreach ($file in $files) {
    Assert-Hash (Join-Path $generationRoot $file.Name) $file.Hash
}
Invoke-NativeChecked -Path (Join-Path $generationRoot 'OmniphonySpatialProbeSmoke.exe') -Arguments @((Join-Path $generationRoot 'OmniphonySpatialProbe.dll')) -Label 'Final-path provider capability smoke'
Invoke-NativeChecked -Path (Join-Path $generationRoot 'OmniphonySpatialStaticStreamSmoke.exe') -Label 'Final-path spatial static stream lifecycle smoke'
Invoke-NativeChecked -Path (Join-Path $generationRoot 'OmniphonySpatialObjectStreamSmoke.exe') -Label 'Final-path spatial dynamic object lifecycle smoke'
Invoke-NativeChecked -Path (Join-Path $generationRoot 'OmniphonySpatialObjectRealtimeSmoke.exe') -Arguments @((Join-Path $generationRoot 'omniphony_realtime.dll')) -Label 'Final-path spatial dynamic object ABI 0.7 smoke'
Invoke-NativeChecked -Path (Join-Path $generationRoot 'OmniphonySpatialRealtimeBridgeSmoke.exe') -Arguments @((Join-Path $generationRoot 'omniphony_realtime.dll')) -Label 'Final-path composed static/dynamic realtime bridge smoke'
Invoke-NativeChecked -Path (Join-Path $generationRoot 'OmniphonySpatialStereoQueueSmoke.exe') -Label 'Final-path stereo clock-domain queue smoke'

$fileHashes = [ordered]@{}
foreach ($file in ($files | Sort-Object Name)) {
    $fileHashes[$file.Name] = $file.Hash
}

$manifest = [ordered]@{
    schema = 'omniphony.windows.spatial-provider-stage.v1'
    state = 'staged-not-registered'
    generation = $generation
    package_sha256 = $packageDigest
    app_root = $AppRoot
    generation_root = $generationRoot
    provider_dll = (Join-Path $generationRoot 'OmniphonySpatialProbe.dll')
    provider_sha256 = $providerHash
    realtime_dll = (Join-Path $generationRoot 'omniphony_realtime.dll')
    realtime_sha256 = $runtimeHash
    object_stream_smoke = (Join-Path $generationRoot 'OmniphonySpatialObjectStreamSmoke.exe')
    object_realtime_smoke = (Join-Path $generationRoot 'OmniphonySpatialObjectRealtimeSmoke.exe')
    stereo_queue_smoke = (Join-Path $generationRoot 'OmniphonySpatialStereoQueueSmoke.exe')
    raw_output_probe = (Join-Path $generationRoot 'OmniphonySpatialRawOutputProbe.exe')
    raw_output_sink_probe = (Join-Path $generationRoot 'OmniphonySpatialRawOutputSinkProbe.exe')
    file_sha256 = $fileHashes
    staged_utc = [DateTime]::UtcNow.ToString('o')
    os_64_bit = [Environment]::Is64BitOperatingSystem
    process_64_bit = [Environment]::Is64BitProcess
    exact_file_set_verified = $true
    final_path_smokes_verified = $true
    dynamic_object_contract_verified = $true
    spatial_object_abi_reset_verified = $true
    composed_dynamic_render_path_verified = $true
    clock_domain_queue_verified = $true
    registry_mutated = $false
    provider_selected = $false
}

New-Item -ItemType Directory -Force -Path $spatialRoot | Out-Null
$tempManifest = "$manifestPath.tmp-$PID"
try {
    $manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $tempManifest -Encoding UTF8
    Move-Item -LiteralPath $tempManifest -Destination $manifestPath -Force
}
finally {
    if (Test-Path -LiteralPath $tempManifest -PathType Leaf) {
        Remove-Item -LiteralPath $tempManifest -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "SPATIAL_PROVIDER_STAGE_OK GENERATION=$generation PACKAGE_SHA256=$packageDigest"
Write-Host "SPATIAL_PROVIDER_STAGE_MANIFEST $manifestPath"
Write-Host 'SPATIAL_PROVIDER_EXACT_FILE_SET_VERIFIED 1'
Write-Host 'SPATIAL_PROVIDER_FINAL_PATH_SMOKES_VERIFIED 1'
Write-Host 'SPATIAL_PROVIDER_DYNAMIC_OBJECT_CONTRACT_VERIFIED 1'
Write-Host 'SPATIAL_PROVIDER_OBJECT_ABI_RESET_VERIFIED 1'
Write-Host 'SPATIAL_PROVIDER_COMPOSED_DYNAMIC_PATH_VERIFIED 1'
Write-Host 'SPATIAL_PROVIDER_CLOCK_DOMAIN_QUEUE_VERIFIED 1'
Write-Host 'SPATIAL_PROVIDER_RAW_OUTPUT_PREFLIGHT_STAGED 1'
Write-Host 'SPATIAL_PROVIDER_RAW_OUTPUT_SINK_PROBE_STAGED 1'
Write-Host 'SPATIAL_PROVIDER_REGISTRY_MUTATED 0'
Write-Host 'SPATIAL_PROVIDER_SELECTED 0'
