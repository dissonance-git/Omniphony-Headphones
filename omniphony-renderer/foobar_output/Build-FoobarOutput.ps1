param(
    [string]$WorkRoot = "",
    [string]$OutputRoot = ""
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Here = $PSScriptRoot
$RepoRoot = (Resolve-Path (Join-Path $Here '..\..')).Path
$RendererRoot = (Resolve-Path (Join-Path $Here '..')).Path
if ([string]::IsNullOrWhiteSpace($WorkRoot)) {
    $WorkRoot = Join-Path $RepoRoot 'build\foobar-output'
}
if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
    $OutputRoot = Join-Path $RepoRoot 'dist\foobar-output'
}

$SdkDate = '2025-03-07'
$SdkUrl = "https://www.foobar2000.org/downloads/SDK-$SdkDate.7z"
$SdkSha256 = 'ccda3c5840e66e0e28a7e4fe36407c4e78581aa30c40c362a188fcbaae799a3e'
$RustToolchain = '1.88.0'

function Need-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command is not on PATH: $Name"
    }
}

function Run([string]$Exe, [string[]]$CommandArgs, [string]$WorkingDirectory = '') {
    if ($WorkingDirectory) { Push-Location $WorkingDirectory }
    try {
        & $Exe @CommandArgs
        if ($LASTEXITCODE -ne 0) {
            throw "$Exe failed with exit code ${LASTEXITCODE}: $($CommandArgs -join ' ')"
        }
    } finally {
        if ($WorkingDirectory) { Pop-Location }
    }
}

function Get-PEMachine([string]$Path) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        if ($reader.ReadUInt16() -ne 0x5A4D) { throw "Not an MZ executable: $Path" }
        $stream.Position = 0x3C
        $peOffset = $reader.ReadInt32()
        $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne 0x00004550) { throw "Missing PE signature: $Path" }
        return [int]$reader.ReadUInt16()
    } finally {
        $reader.Dispose()
        $stream.Dispose()
    }
}

foreach ($command in @('7z', 'cargo', 'rustup')) { Need-Command $command }
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Visual Studio 2022 was not found' }
$msbuild = & $vswhere -latest -products * -requires Microsoft.Component.MSBuild -find 'MSBuild\**\Bin\MSBuild.exe' | Select-Object -First 1
if (-not $msbuild) { throw 'MSBuild was not found' }

Remove-Item -LiteralPath $WorkRoot -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $OutputRoot -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $WorkRoot, $OutputRoot -Force | Out-Null

$sdkArchive = Join-Path $WorkRoot "SDK-$SdkDate.7z"
Invoke-WebRequest -Uri $SdkUrl -OutFile $sdkArchive -UseBasicParsing
$sdkHash = (Get-FileHash -LiteralPath $sdkArchive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($sdkHash -ne $SdkSha256) {
    throw "Foobar SDK SHA-256 mismatch: expected $SdkSha256 got $sdkHash"
}
$sdkRoot = Join-Path $WorkRoot 'sdk'
New-Item -ItemType Directory -Path $sdkRoot | Out-Null
Run '7z' @('x', $sdkArchive, "-o$sdkRoot", '-y')

rustup toolchain install $RustToolchain --profile minimal
if ($LASTEXITCODE -ne 0) { throw 'Rust toolchain installation failed' }

# The output owns two mutually exclusive renderer entries: ordinary stereo
# Current and the recovered-source FullSphere session. Validate the source ABI
# before packaging either side of that switch.
Run 'cargo' @("+$RustToolchain", 'test', '-p', 'source_ffi', '--lib') $RendererRoot
Run 'cargo' @("+$RustToolchain", 'test', '-p', 'source_ffi', '--test', 'abi_layout') $RendererRoot
Run 'cargo' @("+$RustToolchain", 'test', '-p', 'source_ffi', '--test', 'runtime_spatial_mode') $RendererRoot
Run 'cargo' @("+$RustToolchain", 'build', '--release', '-p', 'realtime_ffi') $RendererRoot
Run 'cargo' @("+$RustToolchain", 'build', '--profile', 'release-deploy', '-p', 'source_ffi') $RendererRoot
$realtime = Join-Path $RendererRoot 'target\release\omniphony_realtime.dll'
$source = Join-Path $RendererRoot 'target\release-deploy\omniphony_source.dll'
if (-not (Test-Path -LiteralPath $realtime -PathType Leaf)) {
    throw "Realtime DLL missing: $realtime"
}
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
    throw "Source DLL missing: $source"
}

$componentOut = Join-Path $WorkRoot 'component-out'
New-Item -ItemType Directory -Path $componentOut | Out-Null
$project = Join-Path $Here 'foo_out_omniphony.vcxproj'
$outArg = '/p:OutDir=' + $componentOut + '\'
$sdkArg = '/p:FoobarSdkRoot=' + $sdkRoot
Run $msbuild @(
    $project,
    '/p:Configuration=Release',
    '/p:Platform=x64',
    '/p:PlatformToolset=v143',
    $sdkArg,
    $outArg,
    '/m',
    '/v:m'
)

$component = Join-Path $componentOut 'foo_out_omniphony.dll'
if (-not (Test-Path -LiteralPath $component -PathType Leaf)) {
    throw "Foobar output DLL missing: $component"
}
foreach ($image in @($component, $realtime, $source)) {
    $machine = Get-PEMachine $image
    if ($machine -ne 0x8664) {
        throw ("x64 machine mismatch 0x{0:X4}: {1}" -f $machine, $image)
    }
}

$stage = Join-Path $WorkRoot 'package'
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item -LiteralPath $component -Destination (Join-Path $stage 'foo_out_omniphony.dll')
Copy-Item -LiteralPath $realtime -Destination (Join-Path $stage 'omniphony_realtime.dll')
Copy-Item -LiteralPath $source -Destination (Join-Path $stage 'omniphony_source.dll')
$package = Join-Path $OutputRoot 'foo_out_omniphony.fb2k-component'
Push-Location $stage
try { Run '7z' @('a', '-tzip', '-mx=9', $package, '*') }
finally { Pop-Location }

$hash = (Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash.ToLowerInvariant()
@"
Output: Omniphony diagnostic package

Foobar SDK: $SdkDate
Foobar SDK SHA-256: $SdkSha256
Package SHA-256: $hash

The package contains both ordinary-stereo Current and the recovered-source
FullSphere renderer used by the process-local VGM/SPC source-session ABI.
Source-session substitution is fail-closed: the protected stereo reaching the
output must match the decoder control block before rendered source audio can
replace it.

This package is not listening-approved until the physical shared-RAW,
source-session lifecycle, seek/track-change, and fallback gates pass.
"@ | Set-Content -LiteralPath (Join-Path $OutputRoot 'README.txt') -Encoding UTF8
"$hash  foo_out_omniphony.fb2k-component" |
    Set-Content -LiteralPath (Join-Path $OutputRoot 'SHA256SUMS.txt') -Encoding ASCII

Write-Host "FOOBAR_OUTPUT_PACKAGE $package"
Write-Host "FOOBAR_OUTPUT_SHA256 $hash"
