[CmdletBinding()]
param(
    [string]$ControlPath = '',
    [string]$EndpointControlPath = '',
    [string]$EndpointStatePath = '',
    [string]$ReceiptPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($ControlPath)) {
    $ControlPath = Join-Path $PSScriptRoot 'OmniphonySpatialProbeCtl.exe'
}
if ([string]::IsNullOrWhiteSpace($EndpointControlPath)) {
    $EndpointControlPath = Join-Path $PSScriptRoot 'OmniphonyEndpointCtl.exe'
}
if ([string]::IsNullOrWhiteSpace($EndpointStatePath)) {
    $EndpointStatePath = Join-Path $env:ProgramData 'Omniphony\endpoint-backup.json'
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
    } finally {
        $ErrorActionPreference = $previousPreference
    }
    $lines | ForEach-Object { Write-Host $_ }
    if ($code -ne 0) {
        throw "$Label failed: $Path $($Arguments -join ' ') exit=$code"
    }
    return [string[]]$lines
}

function Add-EndpointId {
    param(
        [Parameter(Mandatory = $true)]
        [System.Collections.Generic.HashSet[string]]$Set,
        [string]$EndpointId
    )
    if (-not [string]::IsNullOrWhiteSpace($EndpointId)) {
        $null = $Set.Add($EndpointId)
    }
}

Assert-Elevated

foreach ($path in @($ControlPath, $EndpointControlPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required Omniphony provider cleanup tool is missing: $path"
    }
}
$ControlPath = (Resolve-Path -LiteralPath $ControlPath).Path
$EndpointControlPath = (Resolve-Path -LiteralPath $EndpointControlPath).Path

$endpointIds = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::OrdinalIgnoreCase
)

$activeEndpoints = Invoke-NativeCaptured -Path $EndpointControlPath -Arguments @('list') -Label 'Enumerate active render endpoints'
foreach ($line in $activeEndpoints) {
    if ($line -match '^ENDPOINT\t([^\t]+)\t') {
        Add-EndpointId -Set $endpointIds -EndpointId $Matches[1]
    }
}

if (Test-Path -LiteralPath $EndpointStatePath -PathType Leaf) {
    $endpointState = Get-Content -LiteralPath $EndpointStatePath -Raw | ConvertFrom-Json
    Add-EndpointId -Set $endpointIds -EndpointId ([string]$endpointState.EndpointId)
}
if (Test-Path -LiteralPath $ReceiptPath -PathType Leaf) {
    $previousReceipt = Get-Content -LiteralPath $ReceiptPath -Raw | ConvertFrom-Json
    if ($previousReceipt.PSObject.Properties.Name -contains 'EndpointId') {
        Add-EndpointId -Set $endpointIds -EndpointId ([string]$previousReceipt.EndpointId)
    }
}

if ($endpointIds.Count -eq 0) {
    throw 'Provider cleanup cannot prove a safe selection state because no endpoint identity is available.'
}

$checkedEndpointIds = @()
foreach ($endpointId in $endpointIds) {
    $selection = Invoke-NativeCaptured -Path $ControlPath -Arguments @('selection-status', $endpointId) -Label "Read spatial selection for $endpointId"
    $selectionText = $selection -join [Environment]::NewLine
    $checkedEndpointIds += $endpointId
    if ($selectionText -match 'OMNIPHONY_DEFAULT\s+1' -or
        $selectionText -match 'OMNIPHONY_ACTIVE\s+1') {
        throw "Omniphony is still the default or active Windows Spatial Sound format on endpoint $endpointId. Provider cleanup stopped before changing the runtime gate or registration. Switch that endpoint's Spatial sound setting away from Omniphony (for example, Off), then run cleanup again."
    }
}

Write-Host 'OMNIPHONY_SPATIAL_SELECTION_CLEANUP_GUARD 1'

$null = Invoke-NativeCaptured -Path $ControlPath -Arguments @('runtime-disable') -Label 'Disable provider runtime gate'
$null = Invoke-NativeCaptured -Path $ControlPath -Arguments @('unregister') -Label 'Unregister Omniphony spatial provider'

$stateRoot = Split-Path -Parent $ReceiptPath
if ($stateRoot) {
    New-Item -ItemType Directory -Force -Path $stateRoot | Out-Null
}
$receipt = [ordered]@{
    SchemaVersion = 2
    TimestampUtc = [DateTime]::UtcNow.ToString('o')
    Enabled = $false
    ProviderRegistered = $false
    SelectionGuardVerified = $true
    CheckedEndpointIds = $checkedEndpointIds
    SelectionChangedByScript = $false
}
$receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ReceiptPath -Encoding UTF8

Write-Host ''
Write-Host 'OMNIPHONY_SPATIAL_PROVIDER_ENABLED 0'
Write-Host 'Omniphony provider registration was removed only after Windows selection readback was clean.'
Write-Host "Receipt: $ReceiptPath"
