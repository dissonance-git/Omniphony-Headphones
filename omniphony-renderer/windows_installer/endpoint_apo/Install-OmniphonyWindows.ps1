param(
    [string]$PackageRoot = '',
    [string]$AppRoot = '',
    [switch]$AllowUnprotectedAudioDG
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($PackageRoot)) { $PackageRoot = $here }
if ([string]::IsNullOrWhiteSpace($AppRoot)) { $AppRoot = Join-Path $env:ProgramFiles 'Omniphony' }

$baselineInstaller = Join-Path $here 'Install-OmniphonyAPO.ps1'
$runtimeRoot = Join-Path $AppRoot 'APO'
$packageStreamApo = Join-Path $PackageRoot 'OmniphonyStreamAPO.dll'
$packageStreamSmoke = Join-Path $PackageRoot 'OmniphonyStreamApoSmoke.exe'
$installedStreamApo = Join-Path $runtimeRoot 'OmniphonyStreamAPO.dll'
$stateRoot = Join-Path $env:ProgramData 'Omniphony'
$endpointBackupPath = Join-Path $stateRoot 'endpoint-backup.json'
$legacyStreamBackupPath = Join-Path $stateRoot 'stream-backup.json'
$logPath = Join-Path $stateRoot 'install-last.log'
$spatialStatePath = Join-Path $stateRoot 'spatial-state-last.log'
$ctl = Join-Path $PackageRoot 'OmniphonyApoCtl.exe'
$endpointCtl = Join-Path $PackageRoot 'OmniphonyEndpointCtl.exe'
$mixProbe = Join-Path $PackageRoot 'OmniphonyMixProbe.exe'
$spatialProbe = Join-Path $here 'OmniphonySpatialProbe.exe'
$spatialProviderProbe = Join-Path $here 'OmniphonySpatialProviderProbe.exe'

foreach ($path in @($baselineInstaller, $packageStreamApo, $packageStreamSmoke, $ctl, $endpointCtl, $mixProbe)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing Omniphony package file: $path" }
}

function Invoke-NativeCapture([string]$Path, [string[]]$Arguments) {
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $lines = @(& $Path @Arguments 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
        if ($null -eq $code) { $code = 0 }
        return [pscustomobject]@{ Code = [int]$code; Lines = [string[]]$lines }
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
}

function Capture-SpatialIngressObservation {
    try {
        New-Item -ItemType Directory -Force -Path $stateRoot | Out-Null
        $lines = @(
            'schema=omniphony.windows.spatial-state.v1',
            "captured_utc=$([DateTime]::UtcNow.ToString('o'))",
            'scope=read_only_endpoint_and_provider_observation',
            'nonclaim=does_not_prove_windows_provider_selection_or_object_delivery'
        )

        $probes = @(
            @{ Label = 'endpoint-capability'; Path = $spatialProbe; Args = @() },
            @{ Label = 'provider-inventory'; Path = $spatialProviderProbe; Args = @() },
            @{ Label = 'provider-com-canary'; Path = $spatialProviderProbe; Args = @('--probe-com') }
        )

        foreach ($probe in $probes) {
            $lines += "=== $($probe.Label) ==="
            if (-not (Test-Path -LiteralPath $probe.Path)) {
                $lines += 'probe_status=missing'
                $lines += "probe_path=$($probe.Path)"
                continue
            }

            $output = @(& $probe.Path @($probe.Args) 2>&1 | ForEach-Object { "$_" })
            $code = $LASTEXITCODE
            if ($null -eq $code) { $code = 0 }
            $lines += "probe_exit_code=$code"
            $lines += $output
        }

        $lines | Set-Content -LiteralPath $spatialStatePath -Encoding UTF8
        Write-Host "SPATIAL_INGRESS_OBSERVATION $spatialStatePath"
    }
    catch {
        Write-Warning "SPATIAL_INGRESS_OBSERVATION_FAILED: $($_.Exception.Message)"
    }
}

function Wait-ServiceState([string]$Name, [System.ServiceProcess.ServiceControllerStatus]$State) {
    $service = Get-Service -Name $Name -ErrorAction Stop
    $service.WaitForStatus($State, [TimeSpan]::FromSeconds(10))
    $service.Refresh()
    if ($service.Status -ne $State) {
        throw "Windows service '$Name' did not reach state '$State'. observed=$($service.Status)"
    }
}

function Set-AudioServiceRunning([bool]$Running) {
    if ($Running) {
        $builder = Get-Service -Name AudioEndpointBuilder -ErrorAction Stop
        if ($builder.Status -ne 'Running') { Start-Service -Name AudioEndpointBuilder }
        Wait-ServiceState 'AudioEndpointBuilder' ([System.ServiceProcess.ServiceControllerStatus]::Running)

        $service = Get-Service -Name AudioSrv -ErrorAction Stop
        if ($service.Status -ne 'Running') { Start-Service -Name AudioSrv }
        Wait-ServiceState 'AudioSrv' ([System.ServiceProcess.ServiceControllerStatus]::Running)
        Start-Sleep -Milliseconds 500
        return
    }

    $service = Get-Service -Name AudioSrv -ErrorAction Stop
    if ($service.Status -ne 'Stopped') { Stop-Service -Name AudioSrv -Force }
    Wait-ServiceState 'AudioSrv' ([System.ServiceProcess.ServiceControllerStatus]::Stopped)
}

function Restart-AudioGraph {
    Write-Host 'AUDIO_GRAPH_RESET_BEGIN native-surround'
    Set-AudioServiceRunning $false
    Start-Sleep -Milliseconds 250
    Set-AudioServiceRunning $true
    Write-Host 'AUDIO_GRAPH_RESET_OK native-surround'
}

function Wait-EndpointActive([string]$ExpectedId) {
    $last = ''
    $reasserted = $false
    for ($attempt = 1; $attempt -le 6; $attempt++) {
        $probe = Invoke-NativeCapture $endpointCtl @('get-default')
        $last = "helper=$($probe.Code) output=$($probe.Lines -join ' | ')"
        $line = $probe.Lines | Where-Object { $_.StartsWith("DEFAULT`t") } | Select-Object -First 1
        if ($probe.Code -eq 0 -and $line) {
            $parts = $line -split "`t", 3
            if ($parts.Count -ge 2 -and [string]::Equals($parts[1], $ExpectedId, [StringComparison]::OrdinalIgnoreCase)) {
                Write-Host "NATIVE_SURROUND_ENDPOINT_ACTIVE_OK ATTEMPT=$attempt ID=$ExpectedId"
                return
            }
        }

        if (-not $reasserted) {
            Write-Warning "NATIVE_SURROUND_ENDPOINT_RECOVERY reassert-default ID=$ExpectedId; $last"
            $setResult = Invoke-NativeCapture $endpointCtl @('set-default-id', $ExpectedId)
            $setResult.Lines | ForEach-Object { Write-Host $_ }
            Restart-AudioGraph
            $reasserted = $true
        }
        else {
            Start-Sleep -Milliseconds 500
        }
    }
    throw "Native-surround endpoint did not return ACTIVE before SFX mutation. endpoint=$ExpectedId $last"
}

function Get-MixChannelCount([string]$EndpointName) {
    $lines = @(& $mixProbe $EndpointName 2>&1 | ForEach-Object { "$_" })
    $code = $LASTEXITCODE
    $lines | ForEach-Object { Write-Host $_ }
    if ($code -ne 0) {
        throw "Physical endpoint mix probe failed: $code"
    }

    $line = $lines | Where-Object { $_.StartsWith("MIX_FORMAT_OK`t") } | Select-Object -First 1
    if (-not $line) {
        throw 'Mix probe returned no MIX_FORMAT_OK record.'
    }

    $match = [regex]::Match($line, '(?:^|\t)CHANNELS=(\d+)(?:\t|$)')
    if (-not $match.Success) {
        throw "Mix probe did not expose a channel count: $line"
    }
    return [int]$match.Groups[1].Value
}

function Assert-NativeSurroundClientFormat([string]$EndpointName) {
    $probe = Invoke-NativeCapture $mixProbe @('--shared-7.1', $EndpointName)
    $probe.Lines | ForEach-Object { Write-Host $_ }
    if ($probe.Code -ne 0) {
        throw "Windows could not initialize an authored 7.1 shared stream through Omniphony's SFX. helper=$($probe.Code) output=$($probe.Lines -join ' | ')"
    }

    $mixLine = $probe.Lines | Where-Object { $_.StartsWith("MIX_FORMAT_OK`t") } | Select-Object -First 1
    if (-not $mixLine) {
        throw 'Native-surround client probe returned no endpoint MIX_FORMAT_OK record.'
    }
    $mixMatch = [regex]::Match($mixLine, '(?:^|\t)CHANNELS=(\d+)(?:\t|$)')
    if (-not $mixMatch.Success -or [int]$mixMatch.Groups[1].Value -lt 1) {
        throw "Native-surround client probe did not expose a valid endpoint mix width: $mixLine"
    }
    $endpointChannels = [int]$mixMatch.Groups[1].Value

    $clientLine = $probe.Lines | Where-Object { $_.StartsWith("SHARED_7_1_INITIALIZE_OK`t") } | Select-Object -First 1
    if (-not $clientLine) {
        throw "Native-surround client probe did not prove 7.1 shared-stream initialization. output=$($probe.Lines -join ' | ')"
    }

    Write-Host "NATIVE_SURROUND_CLIENT_FORMAT_OK INPUT_CHANNELS=8 ENDPOINT_CHANNELS=$endpointChannels RATE=48000 BITS=32"
}

function Assert-RollbackMixFormat([string]$EndpointName) {
    $channels = Get-MixChannelCount $EndpointName
    if ($channels -lt 1) {
        throw "Rollback endpoint mix probe returned an invalid channel count. observed_channels=$channels"
    }
    Write-Host "ROLLBACK_MIX_FORMAT_OK CHANNELS=$channels"
}

function Register-NativeApo {
    $regsvr32 = Join-Path $env:WINDIR 'System32\regsvr32.exe'
    $quotedDll = "`"$installedStreamApo`""
    $process = Start-Process -FilePath $regsvr32 -ArgumentList @('/s', $quotedDll) -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Native-surround APO registration failed: $($process.ExitCode)" }
    Write-Host 'NATIVE_SURROUND_APO_REGISTERED 1'
}

function Unregister-NativeApo {
    if (-not (Test-Path -LiteralPath $installedStreamApo)) { return }
    $regsvr32 = Join-Path $env:WINDIR 'System32\regsvr32.exe'
    $quotedDll = "`"$installedStreamApo`""
    $process = Start-Process -FilePath $regsvr32 -ArgumentList @('/u', '/s', $quotedDll) -Wait -PassThru
    if ($process.ExitCode -ne 0) { Write-Warning "Native-surround APO unregister returned $($process.ExitCode)" }
    else { Write-Host 'NATIVE_SURROUND_APO_REGISTERED 0' }
}

function Assert-ApoCtlIdDispatch {
    # Expected fake-ID failures are captured with native stderr non-terminating.
    # Exit code 3 plus ENDPOINT_NOT_FOUND proves each -id command reached its ID
    # resolver without mutating a real endpoint.
    $fakeEndpointId = '{0.0.0.00000000}.{00000000-0000-0000-0000-000000000000}'
    foreach ($command in @('cleanup-native-sfx-id', 'attach-native-sfx-id', 'detach-id', 'attach-id')) {
        $result = Invoke-NativeCapture $ctl @($command, $fakeEndpointId)
        $idPath = $result.Lines | Where-Object { $_ -eq "ERROR`tENDPOINT_NOT_FOUND" } | Select-Object -First 1
        if ($result.Code -ne 3 -or -not $idPath) {
            throw "OmniphonyApoCtl command dispatch contract failed for '$command'. exit=$($result.Code) output=$($result.Lines -join ' | ')"
        }
    }
    Write-Host 'APO_CTL_ID_DISPATCH_OK 1'
}

# Establish the compatibility baseline first. It owns the endpoint backup and
# AudioDG compatibility state; the physical endpoint keeps its native mix geometry.
& $baselineInstaller -PackageRoot $PackageRoot -AppRoot $AppRoot -AllowUnprotectedAudioDG:$AllowUnprotectedAudioDG

# The baseline script owns and closes the first transcript section. Append the
# native-surround stage to the same file so the real machine result, including
# any failure and rollback reason, survives for diagnosis.
$transcriptStarted = $false
try {
    Start-Transcript -Path $logPath -Append | Out-Null
    $transcriptStarted = $true
} catch {
    Write-Warning "Could not append native-surround installer transcript: $($_.Exception.Message)"
}

$endpointId = ''
$endpointName = ''
$nativeRegistered = $false

try {
    Write-Host 'OMNIPHONY_INSTALL_STAGE baseline-stereo-complete'
    Write-Host 'NATIVE_SURROUND_MIGRATION_BEGIN PLACEMENT=stream-sfx INPUT=preferred-7.1 OUTPUT=stereo'

    Assert-ApoCtlIdDispatch
    Capture-SpatialIngressObservation

    if (-not (Test-Path -LiteralPath $endpointBackupPath)) {
        throw "Missing endpoint backup after stereo baseline install: $endpointBackupPath"
    }
    $endpointBackup = Get-Content -LiteralPath $endpointBackupPath -Raw | ConvertFrom-Json
    $endpointId = [string]$endpointBackup.EndpointId
    $endpointName = [string]$endpointBackup.EndpointName
    if ([string]::IsNullOrWhiteSpace($endpointId) -or [string]::IsNullOrWhiteSpace($endpointName)) {
        throw 'Endpoint backup did not contain a stable endpoint identity.'
    }

    Set-AudioServiceRunning $false
    try {
        Copy-Item -LiteralPath $packageStreamApo -Destination $installedStreamApo -Force
        Register-NativeApo
        $nativeRegistered = $true
    } finally {
        Set-AudioServiceRunning $true
    }

    & $packageStreamSmoke
    if ($LASTEXITCODE -ne 0) { throw "Native-surround APO smoke failed: $LASTEXITCODE" }
    Write-Host 'NATIVE_SURROUND_APO_SMOKE_OK 1'

    # Registration restarts AudioSrv, and Core Audio can publish its endpoint
    # collection slightly later than the service reports Running. Do not touch
    # endpoint policy until the exact baseline endpoint is ACTIVE again.
    Wait-EndpointActive $endpointId

    # Normalize any interrupted older attempt, then install the format-changing
    # APO in the documented per-stream channel-conversion slot. This is the same
    # class of placement Windows documents for headphone virtualization: apps can
    # receive a 7.1 preferred format while the SFX reduces the stream to stereo
    # before the physical endpoint mix.
    & $ctl cleanup-native-sfx-id $endpointId
    if ($LASTEXITCODE -ne 0) { throw "Native-surround SFX cleanup failed: $LASTEXITCODE" }

    & $ctl attach-native-sfx-id $endpointId
    if ($LASTEXITCODE -ne 0) { throw "Native-surround SFX attachment failed: $LASTEXITCODE" }

    # The baseline endpoint EFX and the native SFX both run Current. Once the SFX
    # is attached, remove the rollback EFX before restarting the graph so audio is
    # processed exactly once.
    & $ctl detach-id $endpointId
    if ($LASTEXITCODE -ne 0) { throw "Could not remove stereo rollback EFX after SFX promotion: $LASTEXITCODE" }

    Restart-AudioGraph
    Wait-EndpointActive $endpointId

    # GetMixFormat remains the physical/shared engine mix and is endpoint-owned;
    # it may legitimately be multichannel. The preferred-format contract is upstream
    # of that mix. Prove the real capability by constructing an exact 7.1 float32
    # shared client stream; successful Initialize means the graph builder accepted
    # authored 7.1 through Omniphony's Stream SFX.
    Assert-NativeSurroundClientFormat $endpointName

    if (Test-Path -LiteralPath $legacyStreamBackupPath) {
        Remove-Item -LiteralPath $legacyStreamBackupPath -Force -ErrorAction SilentlyContinue
    }

    Write-Host 'OMNIPHONY_WINDOWS_INSTALL_OK 1'
    Write-Host 'AUDIO_INGRESS windows-client-input=7.1 endpoint-mix=endpoint-native multichannel=authored-speaker-bed output=binaural-stereo'
    Write-Host 'NATIVE_SURROUND_SFX 1'
    Write-Host 'NATIVE_SURROUND_EFX 0'
    Write-Host 'OMNIPHONY_INSTALL_STAGE native-surround-active'
}
catch {
    $failure = $_
    Write-Warning "NATIVE_SURROUND_MIGRATION_FAILED: $($failure.Exception.Message)"

    try {
        if (-not [string]::IsNullOrWhiteSpace($endpointId)) {
            # Rollback also waits for endpoint identity before touching policy.
            Wait-EndpointActive $endpointId

            & $ctl cleanup-native-sfx-id $endpointId
            if ($LASTEXITCODE -ne 0) { throw "Could not remove failed native-surround SFX: $LASTEXITCODE" }

            & $ctl attach-id $endpointId
            if ($LASTEXITCODE -ne 0) { throw "Could not restore stereo Current endpoint APO: $LASTEXITCODE" }
            Restart-AudioGraph
            Wait-EndpointActive $endpointId
            Assert-RollbackMixFormat $endpointName
        }

        if ($nativeRegistered) {
            Set-AudioServiceRunning $false
            try { Unregister-NativeApo }
            finally { Set-AudioServiceRunning $true }
        }

        if (Test-Path -LiteralPath $legacyStreamBackupPath) {
            Remove-Item -LiteralPath $legacyStreamBackupPath -Force -ErrorAction SilentlyContinue
        }
    }
    catch {
        throw "Native-surround migration failed and stereo rollback also failed: $($_.Exception.Message)"
    }

    Write-Host 'OMNIPHONY_WINDOWS_INSTALL_OK 1'
    Write-Host 'NATIVE_SURROUND_SFX 0'
    Write-Host 'NATIVE_SURROUND_EFX 1'
    Write-Host 'OMNIPHONY_INSTALL_STAGE stereo-current-rollback'
    Write-Host 'Stereo Current baseline restored automatically.'
}
finally {
    if ($transcriptStarted) { try { Stop-Transcript | Out-Null } catch { } }
}
