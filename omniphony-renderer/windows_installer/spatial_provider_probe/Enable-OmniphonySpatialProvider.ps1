[CmdletBinding()]
param(
    [string]$ProviderDll = '',
    [string]$ControlPath = '',
    [string]$RealtimeDll = '',
    [string]$RestartAudioPath = '',
    [string]$EndpointStatePath = '',
    [string]$ReceiptPath = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Windows PowerShell 5.1 can evaluate script parameter defaults before
# $PSScriptRoot is populated. Resolve install-relative defaults only after the
# param block so the shipped helper works on stock Windows PowerShell.
if ([string]::IsNullOrWhiteSpace($ProviderDll)) {
    $ProviderDll = Join-Path $PSScriptRoot 'OmniphonySpatialProbe.dll'
}
if ([string]::IsNullOrWhiteSpace($ControlPath)) {
    $ControlPath = Join-Path $PSScriptRoot 'OmniphonySpatialProbeCtl.exe'
}
if ([string]::IsNullOrWhiteSpace($RealtimeDll)) {
    $RealtimeDll = Join-Path (Split-Path -Parent $PSScriptRoot) 'APO\omniphony_realtime.dll'
}
if ([string]::IsNullOrWhiteSpace($RestartAudioPath)) {
    $RestartAudioPath = Join-Path $PSScriptRoot 'Restart-OmniphonyAudio.ps1'
}
if ([string]::IsNullOrWhiteSpace($EndpointStatePath)) {
    $EndpointStatePath = Join-Path $env:ProgramData 'Omniphony\endpoint-backup.json'
}
if ([string]::IsNullOrWhiteSpace($ReceiptPath)) {
    $ReceiptPath = Join-Path $env:ProgramData 'Omniphony\spatial-provider-last.json'
}

$ConfigPath = 'HKLM:\SOFTWARE\Omniphony\SpatialProvider'
$FormatGuid = '{4BD75423-A66C-4586-B782-1FCBBDF2AE74}'
$ComClsid = '{F3CDF827-20C4-405E-A430-8F739343FC89}'

function Assert-Elevated {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Enable-OmniphonySpatialProvider.ps1 requires an elevated Administrator PowerShell.'
    }
}

function Invoke-NativeChecked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
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
        throw "Native command failed: $Path $($Arguments -join ' ') exit=$code"
    }
    return [string[]]$lines
}

function Write-Receipt([bool]$SelectionVerified) {
    $stateRoot = Split-Path -Parent $ReceiptPath
    if ($stateRoot) {
        New-Item -ItemType Directory -Force -Path $stateRoot | Out-Null
    }
    $receipt = [ordered]@{
        SchemaVersion = 2
        TimestampUtc = [DateTime]::UtcNow.ToString('o')
        Enabled = $true
        StaticObjects = $true
        DynamicObjects = $true
        MaxDynamicObjects = 16
        EndpointId = $endpointId
        EndpointName = $endpointName
        ProviderDll = $ProviderDll
        RealtimeDll = $RealtimeDll
        ControlPath = $ControlPath
        FormatGuid = $FormatGuid
        ComClsid = $ComClsid
        SelectionApi = 'Windows.Media.Audio.SpatialAudioDeviceConfiguration'
        SelectionChangedByScript = $true
        SelectionVerified = $SelectionVerified
        WindowsSettingsRequired = $false
        UndocumentedEndpointFormatWrites = $false
    }
    $receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $ReceiptPath -Encoding UTF8
}

Assert-Elevated

foreach ($path in @($ProviderDll, $ControlPath, $RealtimeDll, $RestartAudioPath, $EndpointStatePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required Omniphony spatial-provider file is missing: $path"
    }
}

$endpointState = Get-Content -LiteralPath $EndpointStatePath -Raw | ConvertFrom-Json
$endpointId = [string]$endpointState.EndpointId
$endpointName = [string]$endpointState.EndpointName
if ([string]::IsNullOrWhiteSpace($endpointId)) {
    throw "Endpoint state does not contain EndpointId: $EndpointStatePath"
}

$ProviderDll = (Resolve-Path -LiteralPath $ProviderDll).Path
$ControlPath = (Resolve-Path -LiteralPath $ControlPath).Path
$RealtimeDll = (Resolve-Path -LiteralPath $RealtimeDll).Path
$RestartAudioPath = (Resolve-Path -LiteralPath $RestartAudioPath).Path

$registered = $false
$selectionCommitted = $false
try {
    # Keep the public renderer fail-closed until registration has been verified.
    New-Item -Path $ConfigPath -Force | Out-Null
    New-ItemProperty -Path $ConfigPath -Name Enabled -PropertyType DWord -Value 0 -Force | Out-Null
    New-ItemProperty -Path $ConfigPath -Name EndpointId -PropertyType String -Value $endpointId -Force | Out-Null
    New-ItemProperty -Path $ConfigPath -Name RealtimeDll -PropertyType String -Value $RealtimeDll -Force | Out-Null

    Write-Host 'Registering Omniphony Spatial Sound provider...'
    $null = Invoke-NativeChecked -Path $ControlPath -Arguments @('register', $ProviderDll)
    $registered = $true

    Write-Host 'Verifying provider registration and COM construction...'
    $null = Invoke-NativeChecked -Path $ControlPath -Arguments @('diagnose')

    # The provider must be live before Windows is asked to make it active.
    Set-ItemProperty -Path $ConfigPath -Name Enabled -Type DWord -Value 1

    Write-Host "Selecting Omniphony headlessly for: $endpointName"
    $null = Invoke-NativeChecked -Path $ControlPath -Arguments @('selection-select', $endpointId)
    $selectionCommitted = $true

    # Force the audio graph to reopen so ActiveSpatialAudioFormat catches up to
    # the new verified default format instead of relying on Settings or cache.
    & $RestartAudioPath

    Write-Host 'Verifying Windows active spatial format...'
    $null = Invoke-NativeChecked -Path $ControlPath -Arguments @('selection-verify', $endpointId)

    Write-Receipt $true

    Write-Host ''
    Write-Host 'OMNIPHONY_SPATIAL_PROVIDER_ENABLED 1'
    Write-Host 'OMNIPHONY_SPATIAL_SELECTION_VERIFIED 1'
    Write-Host "Endpoint: $endpointName"
    Write-Host 'Windows Settings is not required for Omniphony routing.'
    Write-Host "Receipt: $ReceiptPath"
} catch {
    $failure = $_

    if ($selectionCommitted) {
        # Do not unregister a provider after Windows has accepted it as the
        # endpoint default. That could strand Windows on a missing COM class.
        # Leave the provider live and record that active readback is pending.
        try { Write-Receipt $false } catch {}
        Write-Warning 'Windows accepted Omniphony as the default spatial format, so provider registration was retained for safety.'
    } else {
        try {
            if (Test-Path -LiteralPath $ConfigPath) {
                Set-ItemProperty -Path $ConfigPath -Name Enabled -Type DWord -Value 0 -ErrorAction SilentlyContinue
            }
        } catch {}

        if ($registered -and (Test-Path -LiteralPath $ControlPath -PathType Leaf)) {
            try {
                $previousPreference = $ErrorActionPreference
                $ErrorActionPreference = 'Continue'
                & $ControlPath unregister 2>&1 | ForEach-Object { Write-Warning "rollback: $_" }
                $ErrorActionPreference = $previousPreference
            } catch {}
        }
    }

    throw $failure
}
