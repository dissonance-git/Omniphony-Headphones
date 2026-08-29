[CmdletBinding()]
param(
    [int]$DurationMs = 1500,
    [string]$EndpointStatePath = '',
    [string]$SelectionReceiptPath = '',
    [string]$ReceiptPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($EndpointStatePath)) {
    $EndpointStatePath = Join-Path $env:ProgramData 'Omniphony\endpoint-backup.json'
}
if ([string]::IsNullOrWhiteSpace($SelectionReceiptPath)) {
    $SelectionReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-provider-selection-check-last.json'
}
if ([string]::IsNullOrWhiteSpace($ReceiptPath)) {
    $ReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-real-static-object-last.json'
}

if ($DurationMs -lt 250 -or $DurationMs -gt 5000) {
    throw 'DurationMs must be between 250 and 5000.'
}

$control = Join-Path $PSScriptRoot 'OmniphonySpatialProbeCtl.exe'
$client = Join-Path $PSScriptRoot 'OmniphonySpatialStaticObjectClientProbe.exe'

foreach ($path in @($EndpointStatePath, $SelectionReceiptPath, $control, $client)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required real-static-object input is missing: $path"
    }
}

function Invoke-NativeCaptured {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)][string]$Label
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
    if ($code -ne 0) {
        throw "$Label failed with exit code $code."
    }
    return [pscustomobject]@{
        code = [int]$code
        output = [string[]]$lines
    }
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

$endpointState = Get-Content -LiteralPath $EndpointStatePath -Raw | ConvertFrom-Json
$selectionReceipt = Get-Content -LiteralPath $SelectionReceiptPath -Raw | ConvertFrom-Json

$endpointId = [string]$endpointState.EndpointId
$endpointName = [string]$endpointState.EndpointName
if ([string]::IsNullOrWhiteSpace($endpointId)) {
    throw 'Endpoint state does not contain EndpointId.'
}
if ($selectionReceipt.Pass -ne $true -or
    [string]$selectionReceipt.State -ne 'selected-active-awaiting-manual-off' -or
    $selectionReceipt.SelectionVerified -ne $true -or
    [string]$selectionReceipt.EndpointId -ne $endpointId) {
    throw 'A matching successful controlled-selection receipt is required before real static-object submission.'
}

$selectionBefore = Invoke-NativeCaptured -Path $control -Arguments @('selection-status', $endpointId) -Label 'Pre-source Windows spatial selection'
Require-Pattern -Result $selectionBefore -Pattern 'OMNIPHONY_DEFAULT\s+1' -Label 'Pre-source Windows spatial selection'
Require-Pattern -Result $selectionBefore -Pattern 'OMNIPHONY_ACTIVE\s+1' -Label 'Pre-source Windows spatial selection'

Write-Host ''
Write-Host 'Omniphony real Windows Spatial Audio static-object check'
Write-Host "Endpoint: $endpointName"
Write-Host 'Role: TopFrontLeft'
Write-Host "Duration: $DurationMs ms"
Write-Host 'SPATIAL_REAL_STATIC_OBJECT_WARNING audible-low-level-test-tone'
Write-Host 'This test does not change Windows Spatial Sound selection.'
Write-Host ''

$source = Invoke-NativeCaptured -Path $client -Arguments @($endpointId, [string]$DurationMs) -Label 'Real Windows Spatial Audio TopFrontLeft client'
Require-Pattern -Result $source -Pattern 'SPATIAL_REAL_STATIC_CLIENT_OK\s+1' -Label 'Real Windows Spatial Audio TopFrontLeft client'
Require-Pattern -Result $source -Pattern 'SPATIAL_REAL_STATIC_CLIENT_ROUTE\s+IMMDEVICE_ISPATIALAUDIOCLIENT' -Label 'Real Windows Spatial Audio TopFrontLeft client'
Require-Pattern -Result $source -Pattern 'SPATIAL_REAL_STATIC_CLIENT_ROLE\s+TOP_FRONT_LEFT' -Label 'Real Windows Spatial Audio TopFrontLeft client'
Require-Pattern -Result $source -Pattern 'SPATIAL_REAL_STATIC_CLIENT_OBJECT_ACTIVATED\s+1' -Label 'Real Windows Spatial Audio TopFrontLeft client'
Require-Pattern -Result $source -Pattern 'SPATIAL_REAL_STATIC_CLIENT_END_OF_STREAM\s+1' -Label 'Real Windows Spatial Audio TopFrontLeft client'
Require-Pattern -Result $source -Pattern 'SPATIAL_REAL_STATIC_CLIENT_STREAM_STOPPED\s+1' -Label 'Real Windows Spatial Audio TopFrontLeft client'

$selectionAfter = Invoke-NativeCaptured -Path $control -Arguments @('selection-status', $endpointId) -Label 'Post-source Windows spatial selection'
Require-Pattern -Result $selectionAfter -Pattern 'OMNIPHONY_DEFAULT\s+1' -Label 'Post-source Windows spatial selection'
Require-Pattern -Result $selectionAfter -Pattern 'OMNIPHONY_ACTIVE\s+1' -Label 'Post-source Windows spatial selection'

$receiptRoot = Split-Path -Parent $ReceiptPath
if ($receiptRoot) {
    New-Item -ItemType Directory -Force -Path $receiptRoot | Out-Null
}
$receipt = [ordered]@{
    SchemaVersion = 1
    TimestampUtc = [DateTime]::UtcNow.ToString('o')
    Pass = $true
    State = 'real-static-object-submitted-awaiting-audible-confirmation'
    EndpointId = $endpointId
    EndpointName = $endpointName
    Generation = [string]$selectionReceipt.Generation
    PackageSha256 = [string]$selectionReceipt.PackageSha256
    ClientRoute = 'IMMDevice::Activate(ISpatialAudioClient)'
    StaticRole = 'TopFrontLeft'
    AudioObjectType = 'AudioObjectType_TopFrontLeft'
    Format = 'FLOAT32_48000_MONO'
    ToneHz = 550.0
    DurationMs = $DurationMs
    WindowsDefaultOmniphony = $true
    WindowsActiveOmniphony = $true
    StreamStarted = $true
    ObjectSubmitted = $true
    EndOfStreamSubmitted = $true
    HumanAudibleConfirmationRequired = $true
}
$receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ReceiptPath -Encoding UTF8

Write-Host ''
Write-Host 'SPATIAL_REAL_STATIC_OBJECT_CHECK_OK 1'
Write-Host 'SPATIAL_REAL_STATIC_OBJECT_WINDOWS_SELECTED_PROVIDER 1'
Write-Host 'SPATIAL_REAL_STATIC_OBJECT_ROLE TOP_FRONT_LEFT'
Write-Host 'SPATIAL_REAL_STATIC_OBJECT_HUMAN_AUDIBLE_CONFIRMATION_REQUIRED 1'
Write-Host "Receipt: $ReceiptPath"
Write-Host ''
Write-Host 'After confirming what you heard, set Spatial sound to Off before running selection cleanup.'
