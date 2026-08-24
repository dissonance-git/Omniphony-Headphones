param(
    [string]$EndpointCtl = '',
    [string]$OutputPath = ''
)

$ErrorActionPreference = 'Stop'
$productionRoot = $PSScriptRoot
$lowLevelCapture = Join-Path $productionRoot 'Capture-TargetAudioDriver.ps1'
$finalizer = Join-Path $productionRoot 'Finalize-TargetEvidence.ps1'

foreach ($required in @($lowLevelCapture, $finalizer)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Required production capture helper is missing: $required"
    }
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path (Get-Location) 'omniphony-audio-target.json'
}
$OutputPath = [IO.Path]::GetFullPath($OutputPath)
$tempCapture = Join-Path ([IO.Path]::GetTempPath()) ("omniphony-target-v2-" + [Guid]::NewGuid().ToString('N') + '.json')
$tempEnriched = Join-Path ([IO.Path]::GetTempPath()) ("omniphony-target-v2-enriched-" + [Guid]::NewGuid().ToString('N') + '.json')

function Get-EndpointEffectSnapshot($Capture) {
    $mmDeviceId = [string]$Capture.DefaultEndpoint.MmDeviceId
    if ($mmDeviceId -notmatch '(\{[0-9A-Fa-f-]{36}\})$') {
        return [ordered]@{ Readable = $false; Error = "MMDevice ID has no endpoint GUID tail: $mmDeviceId" }
    }
    $endpointGuid = $Matches[1]
    $fxPath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\$endpointGuid\FxProperties"
    if (-not (Test-Path -LiteralPath $fxPath)) {
        return [ordered]@{ Readable = $false; EndpointGuid = $endpointGuid; RegistryPath = $fxPath; Error = 'FxProperties path not found or not readable' }
    }
    try {
        $item = Get-ItemProperty -LiteralPath $fxPath -ErrorAction Stop
        $legacyName = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},7'
        $compositeName = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},15'
        $disabledName = '{1da5d803-d492-4edd-8c23-e0c0ffee7f0e},5'
        $legacy = @()
        $composite = @()
        if ($item.PSObject.Properties[$legacyName]) {
            $legacy = @($item.PSObject.Properties[$legacyName].Value | ForEach-Object { [string]$_ } | Where-Object { $_ })
        }
        if ($item.PSObject.Properties[$compositeName]) {
            $composite = @($item.PSObject.Properties[$compositeName].Value | ForEach-Object { [string]$_ } | Where-Object { $_ })
        }
        $disabled = 0
        if ($item.PSObject.Properties[$disabledName]) {
            try { $disabled = [int]$item.PSObject.Properties[$disabledName].Value } catch { $disabled = -1 }
        }
        return [ordered]@{
            Readable = $true
            EndpointGuid = $endpointGuid
            RegistryPath = $fxPath
            LegacyEndpointEffects = $legacy
            CompositeEndpointEffects = $composite
            EnhancementsDisabled = $disabled
            Error = ''
        }
    } catch {
        return [ordered]@{ Readable = $false; EndpointGuid = $endpointGuid; RegistryPath = $fxPath; Error = $_.Exception.Message }
    }
}

try {
    $captureArgs = @('-OutputPath', $tempCapture)
    if (-not [string]::IsNullOrWhiteSpace($EndpointCtl)) {
        $captureArgs = @('-EndpointCtl', $EndpointCtl) + $captureArgs
    }
    & $lowLevelCapture @captureArgs
    if (-not (Test-Path -LiteralPath $tempCapture -PathType Leaf)) {
        throw 'Low-level target capture failed.'
    }

    $capture = Get-Content -LiteralPath $tempCapture -Raw | ConvertFrom-Json
    if ([string]$capture.Schema -ne 'omniphony.windows.apo-target.v2') {
        throw "Unexpected low-level target capture schema: $($capture.Schema)"
    }

    foreach ($candidate in @($capture.AssociationCandidates)) {
        $instanceId = [string]$candidate.InstanceId
        $sectionExt = ''
        try {
            $sectionExt = [string](Get-PnpDeviceProperty -InstanceId $instanceId -KeyName 'DEVPKEY_Device_DriverInfSectionExt' -ErrorAction Stop).Data
        } catch {
            Write-Warning "Could not read DEVPKEY_Device_DriverInfSectionExt for ${instanceId}: $($_.Exception.Message)"
        }
        $candidate | Add-Member -NotePropertyName DriverInfSectionExt -NotePropertyValue $sectionExt -Force
    }

    $capture | Add-Member -NotePropertyName CapturedEndpointEffects -NotePropertyValue (Get-EndpointEffectSnapshot $capture) -Force
    $capture | ConvertTo-Json -Depth 18 | Set-Content -LiteralPath $tempEnriched -Encoding UTF8

    & $finalizer -InputJson $tempEnriched -OutputJson $OutputPath
    if (-not (Test-Path -LiteralPath $OutputPath -PathType Leaf)) {
        throw 'Deterministic target evidence finalization failed.'
    }

    $final = Get-Content -LiteralPath $OutputPath -Raw | ConvertFrom-Json
    $candidates = @($final.AssociationCandidates)
    Write-Host "OMNIPHONY_PRODUCTION_TARGET_CAPTURE_OK`t$OutputPath"
    Write-Host "TARGET_SCHEMA`t$($final.Schema)"
    Write-Host "DEFAULT_ENDPOINT`t$($final.DefaultEndpoint.FriendlyName)`t$($final.DefaultEndpoint.MmDeviceId)"
    foreach ($candidate in $candidates) {
        Write-Host "DRIVER_SECTION`t$($candidate.DriverInfSectionBase)`t$($candidate.DriverInfSectionExt)`t$($candidate.DriverInfResolvedSection)"
        foreach ($reference in @($candidate.PairedTopologyReferenceCandidates)) {
            Write-Host "PAIRED_TOPOLOGY_REFERENCE`t$reference"
        }
        foreach ($warning in @($candidate.InterfaceResolutionWarnings)) {
            Write-Warning "TARGET_EVIDENCE $warning"
        }
    }
    if ($final.CapturedEndpointEffects -and -not [bool]$final.CapturedEndpointEffects.Readable) {
        Write-Warning "ENDPOINT_EFFECT_CAPTURE $($final.CapturedEndpointEffects.Error)"
    }
}
finally {
    Remove-Item -LiteralPath $tempCapture -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $tempEnriched -Force -ErrorAction SilentlyContinue
}
