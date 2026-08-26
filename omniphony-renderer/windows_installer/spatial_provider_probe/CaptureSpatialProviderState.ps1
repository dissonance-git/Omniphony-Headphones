[CmdletBinding()]
param(
    [string]$ControlPath = (Join-Path $PSScriptRoot 'OmniphonySpatialProbeCtl.exe'),
    [ValidateRange(1, 12)]
    [int]$MaxDepth = 8,
    [ValidateRange(32, 65536)]
    [int]$MaxValueBytes = 4096
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$EncoderPath = 'SOFTWARE\Microsoft\Multimedia\Audio\Spatial\Encoder'
$ProviderRuntimePath = 'SOFTWARE\Omniphony\SpatialProvider'
$SpatialEndpointPath = 'SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\SpatialAudioEndpoint'
$WindowsVersionPath = 'SOFTWARE\Microsoft\Windows NT\CurrentVersion'

function Write-JsonRecord {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Tag,
        [Parameter(Mandatory = $true)]
        [System.Collections.IDictionary]$Payload
    )

    $json = ([pscustomobject]$Payload | ConvertTo-Json -Compress -Depth 8)
    Write-Output ("{0}`t{1}" -f $Tag, $json)
}

function Get-LimitedString {
    param(
        [AllowNull()]
        [string]$Value,
        [int]$LimitBytes
    )

    if ($null -eq $Value) {
        return [pscustomobject][ordered]@{
            data = $null
            byte_count = 0
            truncated = $false
        }
    }

    $encoding = [System.Text.Encoding]::Unicode
    $byteCount = $encoding.GetByteCount($Value)
    if ($byteCount -le $LimitBytes) {
        return [pscustomobject][ordered]@{
            data = $Value
            byte_count = $byteCount
            truncated = $false
        }
    }

    $maxChars = [Math]::Max(1, [int][Math]::Floor($LimitBytes / 2))
    if ($maxChars -gt $Value.Length) {
        $maxChars = $Value.Length
    }
    $trimmed = $Value.Substring(0, $maxChars)
    return [pscustomobject][ordered]@{
        data = $trimmed
        byte_count = $byteCount
        truncated = $true
    }
}

function Convert-RegistryValue {
    param(
        [AllowNull()]
        [object]$Value,
        [Microsoft.Win32.RegistryValueKind]$Kind,
        [int]$LimitBytes
    )

    switch ($Kind) {
        ([Microsoft.Win32.RegistryValueKind]::Binary) {
            [byte[]]$bytes = if ($Value -is [byte[]]) { $Value } else { [byte[]]@() }
            $take = [Math]::Min($bytes.Length, $LimitBytes)
            $hex = if ($take -gt 0) {
                [BitConverter]::ToString($bytes, 0, $take).Replace('-', '')
            } else {
                ''
            }
            return [pscustomobject][ordered]@{
                data = $hex
                byte_count = $bytes.Length
                truncated = ($take -lt $bytes.Length)
            }
        }
        ([Microsoft.Win32.RegistryValueKind]::DWord) {
            return [pscustomobject][ordered]@{
                data = ('0x{0:X8}' -f [uint32]$Value)
                byte_count = 4
                truncated = $false
            }
        }
        ([Microsoft.Win32.RegistryValueKind]::QWord) {
            return [pscustomobject][ordered]@{
                data = ('0x{0:X16}' -f [uint64]$Value)
                byte_count = 8
                truncated = $false
            }
        }
        ([Microsoft.Win32.RegistryValueKind]::MultiString) {
            $separator = [string][char]0x1F
            $joined = (@($Value | ForEach-Object { [string]$_ }) -join $separator)
            return Get-LimitedString -Value $joined -LimitBytes $LimitBytes
        }
        default {
            return Get-LimitedString -Value ([string]$Value) -LimitBytes $LimitBytes
        }
    }
}

function Write-RegistryTree {
    param(
        [Parameter(Mandatory = $true)]
        [Microsoft.Win32.RegistryKey]$BaseKey,
        [Parameter(Mandatory = $true)]
        [string]$RelativePath,
        [int]$Depth = 0
    )

    if ($Depth -gt $MaxDepth) {
        Write-JsonRecord -Tag 'REGISTRY_DEPTH_LIMIT' -Payload ([ordered]@{
            path = "HKLM\$RelativePath"
            max_depth = $MaxDepth
        })
        return
    }

    try {
        $key = $BaseKey.OpenSubKey($RelativePath, $false)
    } catch {
        Write-JsonRecord -Tag 'REGISTRY_OPEN_ERROR' -Payload ([ordered]@{
            path = "HKLM\$RelativePath"
            error = $_.Exception.Message
        })
        return
    }

    if ($null -eq $key) {
        Write-JsonRecord -Tag 'REGISTRY_MISSING' -Payload ([ordered]@{
            path = "HKLM\$RelativePath"
        })
        return
    }

    try {
        Write-JsonRecord -Tag 'REGISTRY_KEY' -Payload ([ordered]@{
            path = "HKLM\$RelativePath"
            depth = $Depth
        })

        foreach ($valueName in @($key.GetValueNames() | Sort-Object)) {
            try {
                $kind = $key.GetValueKind($valueName)
                $value = $key.GetValue(
                    $valueName,
                    $null,
                    [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
                $encoded = Convert-RegistryValue -Value $value -Kind $kind -LimitBytes $MaxValueBytes
                Write-JsonRecord -Tag 'REGISTRY_VALUE' -Payload ([ordered]@{
                    path = "HKLM\$RelativePath"
                    name = $(if ([string]::IsNullOrEmpty($valueName)) { '(Default)' } else { $valueName })
                    kind = $kind.ToString()
                    data = $encoded.data
                    byte_count = $encoded.byte_count
                    truncated = $encoded.truncated
                })
            } catch {
                Write-JsonRecord -Tag 'REGISTRY_VALUE_ERROR' -Payload ([ordered]@{
                    path = "HKLM\$RelativePath"
                    name = $(if ([string]::IsNullOrEmpty($valueName)) { '(Default)' } else { $valueName })
                    error = $_.Exception.Message
                })
            }
        }

        foreach ($subKeyName in @($key.GetSubKeyNames() | Sort-Object)) {
            $childPath = "$RelativePath\$subKeyName"
            Write-RegistryTree -BaseKey $BaseKey -RelativePath $childPath -Depth ($Depth + 1)
        }
    } finally {
        $key.Dispose()
    }
}

function Write-WindowsVersion {
    param(
        [Parameter(Mandatory = $true)]
        [Microsoft.Win32.RegistryKey]$BaseKey
    )

    $key = $BaseKey.OpenSubKey($WindowsVersionPath, $false)
    if ($null -eq $key) {
        Write-JsonRecord -Tag 'WINDOWS_VERSION' -Payload ([ordered]@{ status = 'unavailable' })
        return
    }

    try {
        Write-JsonRecord -Tag 'WINDOWS_VERSION' -Payload ([ordered]@{
            product_name = [string]$key.GetValue('ProductName', '')
            display_version = [string]$key.GetValue('DisplayVersion', '')
            build = [string]$key.GetValue('CurrentBuildNumber', '')
            ubr = [string]$key.GetValue('UBR', '')
            edition = [string]$key.GetValue('EditionID', '')
        })
    } finally {
        $key.Dispose()
    }
}

Write-Output "SPATIAL_SELECTION_SNAPSHOT_VERSION`t1"
Write-Output "SPATIAL_SELECTION_SNAPSHOT_READ_ONLY`t1"
Write-Output "SPATIAL_SELECTION_SNAPSHOT_NO_MMDEVICES_WRITES`t1"
Write-Output "SPATIAL_SELECTION_SNAPSHOT_MAX_DEPTH`t$MaxDepth"
Write-Output "SPATIAL_SELECTION_SNAPSHOT_MAX_VALUE_BYTES`t$MaxValueBytes"

$base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
    [Microsoft.Win32.RegistryHive]::LocalMachine,
    [Microsoft.Win32.RegistryView]::Registry64)

try {
    Write-WindowsVersion -BaseKey $base

    if (Test-Path -LiteralPath $ControlPath -PathType Leaf) {
        $controlOutput = & $ControlPath list 2>&1
        $controlExit = $LASTEXITCODE
        foreach ($line in @($controlOutput)) {
            Write-Output ("CONTROL_LIST`t{0}" -f [string]$line)
        }
        Write-Output "CONTROL_LIST_EXIT`t$controlExit"
        if ($controlExit -ne 0) {
            throw "OmniphonySpatialProbeCtl list failed with exit code $controlExit"
        }

        $runtimeOutput = & $ControlPath runtime-status 2>&1
        $runtimeExit = $LASTEXITCODE
        foreach ($line in @($runtimeOutput)) {
            Write-Output ("CONTROL_RUNTIME_STATUS`t{0}" -f [string]$line)
        }
        Write-Output "CONTROL_RUNTIME_STATUS_EXIT`t$runtimeExit"
        if ($runtimeExit -ne 0) {
            throw "OmniphonySpatialProbeCtl runtime-status failed with exit code $runtimeExit"
        }
    } else {
        Write-JsonRecord -Tag 'CONTROL_LIST' -Payload ([ordered]@{
            status = 'control_not_found'
            path = $ControlPath
        })
    }

    Write-Output "REGISTRY_SECTION`tSPATIAL_ENCODER"
    Write-RegistryTree -BaseKey $base -RelativePath $EncoderPath

    Write-Output "REGISTRY_SECTION`tOMNIPHONY_PROVIDER_RUNTIME"
    Write-RegistryTree -BaseKey $base -RelativePath $ProviderRuntimePath

    Write-Output "REGISTRY_SECTION`tSPATIAL_AUDIO_ENDPOINT"
    Write-RegistryTree -BaseKey $base -RelativePath $SpatialEndpointPath
} finally {
    $base.Dispose()
}

Write-Output "SPATIAL_SELECTION_SNAPSHOT_COMPLETE`t1"
