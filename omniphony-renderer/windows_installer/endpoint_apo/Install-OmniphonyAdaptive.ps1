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
$restartAudio = Join-Path $here 'Restart-OmniphonyAudio.ps1'
$stateRoot = Join-Path $env:ProgramData 'Omniphony'
$backupPath = Join-Path $stateRoot 'endpoint-backup.json'
$logPath = Join-Path $stateRoot 'install-last.log'
$runtimeRoot = Join-Path $AppRoot 'APO'
$packageStreamApo = Join-Path $PackageRoot 'OmniphonyStreamAPO.dll'
$packageStreamSmoke = Join-Path $PackageRoot 'OmniphonyStreamApoSmoke.exe'
$installedStreamApo = Join-Path $runtimeRoot 'OmniphonyStreamAPO.dll'
$ctl = Join-Path $PackageRoot 'OmniphonyApoCtl.exe'
$endpointCtl = Join-Path $PackageRoot 'OmniphonyEndpointCtl.exe'
$mixProbe = Join-Path $PackageRoot 'OmniphonyMixProbe.exe'
$nativeSfxClsid = '{07D403D9-8A98-43EF-8C28-8651756D83BE}'
$currentEfxClsid = '{A9333BFE-39C1-40FD-B4B0-ECC591410B47}'

foreach ($path in @($baselineInstaller, $restartAudio, $packageStreamApo, $packageStreamSmoke, $ctl, $endpointCtl, $mixProbe)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing Omniphony package file: $path" }
}

function Invoke-Capture([string]$Path, [string[]]$Arguments) {
    $old = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $lines = @(& $Path @Arguments 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
        if ($null -eq $code) { $code = 0 }
        return [pscustomobject]@{ Code = [int]$code; Lines = [string[]]$lines }
    }
    finally { $ErrorActionPreference = $old }
}

function Restart-Graph {
    Write-Host 'AUDIO_GRAPH_RESET_BEGIN adaptive'
    & $restartAudio
    Write-Host 'AUDIO_GRAPH_RESET_OK adaptive'
}

function Wait-Endpoint([string]$ExpectedId) {
    $last = ''
    $reasserted = $false
    for ($attempt = 1; $attempt -le 6; $attempt++) {
        $probe = Invoke-Capture $endpointCtl @('get-default')
        $last = "helper=$($probe.Code) output=$($probe.Lines -join ' | ')"
        $line = $probe.Lines | Where-Object { $_.StartsWith("DEFAULT`t") } | Select-Object -First 1
        if ($probe.Code -eq 0 -and $line) {
            $parts = $line -split "`t", 3
            if ($parts.Count -ge 2 -and [string]::Equals($parts[1], $ExpectedId, [StringComparison]::OrdinalIgnoreCase)) {
                Write-Host "ENDPOINT_ACTIVE_OK ATTEMPT=$attempt ID=$ExpectedId"
                return
            }
        }
        if (-not $reasserted) {
            $set = Invoke-Capture $endpointCtl @('set-default-id', $ExpectedId)
            $set.Lines | ForEach-Object { Write-Host $_ }
            Restart-Graph
            $reasserted = $true
        }
        else { Start-Sleep -Milliseconds 500 }
    }
    throw "Endpoint did not return ACTIVE. endpoint=$ExpectedId $last"
}

function Get-MixSnapshot([string]$EndpointName) {
    $probe = Invoke-Capture $mixProbe @($EndpointName)
    $probe.Lines | ForEach-Object { Write-Host $_ }
    if ($probe.Code -ne 0) { throw "Endpoint mix probe failed: $($probe.Code)" }
    $line = $probe.Lines | Where-Object { $_.StartsWith("MIX_FORMAT_OK`t") } | Select-Object -First 1
    if (-not $line) { throw 'Mix probe returned no MIX_FORMAT_OK record.' }
    $channels = [regex]::Match($line, '(?:^|\t)CHANNELS=(\d+)(?:\t|$)')
    $rate = [regex]::Match($line, '(?:^|\t)RATE=(\d+)(?:\t|$)')
    $bits = [regex]::Match($line, '(?:^|\t)BITS=(\d+)(?:\t|$)')
    if (-not $channels.Success -or -not $rate.Success -or -not $bits.Success) {
        throw "Mix probe did not expose complete geometry: $line"
    }
    return [pscustomobject]@{
        Channels = [int]$channels.Groups[1].Value
        Rate = [int]$rate.Groups[1].Value
        Bits = [int]$bits.Groups[1].Value
    }
}

function Assert-GeometryPreserved($Before, $After) {
    if ($Before.Channels -ne $After.Channels -or $Before.Rate -ne $After.Rate -or $Before.Bits -ne $After.Bits) {
        throw "Endpoint geometry changed during Omniphony migration. before=$($Before.Channels)ch/$($Before.Rate)/$($Before.Bits) after=$($After.Channels)ch/$($After.Rate)/$($After.Bits)"
    }
    Write-Host "ENDPOINT_GEOMETRY_PRESERVED CHANNELS=$($After.Channels) RATE=$($After.Rate) BITS=$($After.Bits)"
}

function Get-FxStatus([string]$EndpointId) {
    $status = Invoke-Capture $ctl @('status-id', $EndpointId)
    $status.Lines | ForEach-Object { Write-Host $_ }
    if ($status.Code -ne 0 -and $status.Code -ne 3) {
        throw "Could not inspect endpoint FX state. helper=$($status.Code)"
    }
    $efxLine = $status.Lines | Where-Object { $_.StartsWith("EFX`t") } | Select-Object -First 1
    $sfxLine = $status.Lines | Where-Object { $_.StartsWith("SFX`t") } | Select-Object -First 1
    $efx = if ($efxLine) { ($efxLine -split "`t", 2)[1] } else { '<unknown>' }
    $sfx = if ($sfxLine) { ($sfxLine -split "`t", 2)[1] } else { '<unknown>' }
    return [pscustomobject]@{ Efx = $efx; Sfx = $sfx }
}

function Set-AudioServiceRunning([bool]$Running) {
    if ($Running) {
        $builder = Get-Service -Name AudioEndpointBuilder -ErrorAction Stop
        if ($builder.Status -ne 'Running') { Start-Service -Name AudioEndpointBuilder }
        $builder.WaitForStatus('Running', [TimeSpan]::FromSeconds(10))
        $audio = Get-Service -Name Audiosrv -ErrorAction Stop
        if ($audio.Status -ne 'Running') { Start-Service -Name Audiosrv }
        $audio.WaitForStatus('Running', [TimeSpan]::FromSeconds(10))
        Start-Sleep -Milliseconds 500
    }
    else {
        $audio = Get-Service -Name Audiosrv -ErrorAction Stop
        if ($audio.Status -ne 'Stopped') { Stop-Service -Name Audiosrv -Force }
        $audio.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(10))
    }
}

# Establish the tested Current endpoint installation first. On multichannel
# endpoint mixes this EFX is intentionally only a rollback anchor; the actual
# renderer must be promoted to the stream SFX because Current itself is stereo.
& $baselineInstaller -PackageRoot $PackageRoot -AppRoot $AppRoot -AllowUnprotectedAudioDG:$AllowUnprotectedAudioDG

$transcriptStarted = $false
try {
    Start-Transcript -Path $logPath -Append | Out-Null
    $transcriptStarted = $true
} catch {
    Write-Warning "Could not append adaptive installer transcript: $($_.Exception.Message)"
}

$endpointId = ''
$endpointName = ''
$nativeRegistered = $false
$before = $null

try {
    if (-not (Test-Path -LiteralPath $backupPath)) { throw "Missing endpoint backup: $backupPath" }
    $backup = Get-Content -LiteralPath $backupPath -Raw | ConvertFrom-Json
    $endpointId = [string]$backup.EndpointId
    $endpointName = [string]$backup.EndpointName
    if ([string]::IsNullOrWhiteSpace($endpointId) -or [string]::IsNullOrWhiteSpace($endpointName)) {
        throw 'Endpoint backup does not contain a stable endpoint identity.'
    }

    Wait-Endpoint $endpointId
    $before = Get-MixSnapshot $endpointName
    Write-Host "OMNIPHONY_ADAPTIVE_MIGRATION_BEGIN ENDPOINT_CHANNELS=$($before.Channels)"

    Set-AudioServiceRunning $false
    try {
        New-Item -ItemType Directory -Force -Path $runtimeRoot | Out-Null
        Copy-Item -LiteralPath $packageStreamApo -Destination $installedStreamApo -Force
        $regsvr32 = Join-Path $env:WINDIR 'System32\regsvr32.exe'
        $register = Start-Process -FilePath $regsvr32 -ArgumentList @('/s', "`"$installedStreamApo`"") -Wait -PassThru
        if ($register.ExitCode -ne 0) { throw "Native stream APO registration failed: $($register.ExitCode)" }
        $nativeRegistered = $true
        Write-Host 'NATIVE_SURROUND_APO_REGISTERED 1'
    }
    finally { Set-AudioServiceRunning $true }

    & $packageStreamSmoke
    if ($LASTEXITCODE -ne 0) { throw "Native stream APO smoke failed: $LASTEXITCODE" }
    Write-Host 'NATIVE_SURROUND_APO_SMOKE_OK 1'

    Wait-Endpoint $endpointId
    & $ctl cleanup-native-sfx-id $endpointId
    if ($LASTEXITCODE -ne 0) { throw "Native SFX cleanup failed: $LASTEXITCODE" }
    & $ctl attach-native-sfx-id $endpointId
    if ($LASTEXITCODE -ne 0) { throw "Native SFX attachment failed: $LASTEXITCODE" }
    & $ctl detach-id $endpointId
    if ($LASTEXITCODE -ne 0) { throw "Could not remove duplicate Current EFX after SFX promotion: $LASTEXITCODE" }

    Restart-Graph
    Wait-Endpoint $endpointId

    $after = Get-MixSnapshot $endpointName
    Assert-GeometryPreserved $before $after

    $fx = Get-FxStatus $endpointId
    if ($fx.Efx -ne '<absent>') { throw "Duplicate endpoint EFX remains attached: $($fx.Efx)" }
    if (-not [string]::Equals($fx.Sfx, $nativeSfxClsid, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Native stream SFX is not the sole Omniphony path. observed_sfx=$($fx.Sfx)"
    }
    Write-Host 'SINGLE_RENDER_PATH_OK EFX=0 SFX=1'

    if ($before.Channels -eq 2) {
        $client = Invoke-Capture $mixProbe @('--shared-7.1', $endpointName)
        $client.Lines | ForEach-Object { Write-Host $_ }
        if ($client.Code -ne 0) { throw "Authored 7.1 shared-stream probe failed: $($client.Code)" }
        Write-Host 'AUTHORED_7_1_SHARED_STREAM_OK 1'
    }
    else {
        Write-Host "AUTHORED_7_1_SHARED_STREAM_PROBE_SKIPPED ENDPOINT_CHANNELS=$($before.Channels) REASON=existing-probe-hardcodes-stereo-floor"
    }

    Write-Host 'OMNIPHONY_WINDOWS_INSTALL_OK 1'
    Write-Host "AUDIO_INGRESS endpoint-mix-channels=$($before.Channels) stream-sfx=current output=binaural-stereo"
    Write-Host 'OMNIPHONY_INSTALL_STAGE adaptive-native-sfx-active'
}
catch {
    $failure = $_
    Write-Warning "OMNIPHONY_ADAPTIVE_MIGRATION_FAILED: $($failure.Exception.Message)"

    try {
        if (-not [string]::IsNullOrWhiteSpace($endpointId)) {
            Wait-Endpoint $endpointId
            & $ctl cleanup-native-sfx-id $endpointId
            if ($LASTEXITCODE -ne 0) { throw "Could not remove failed native SFX: $LASTEXITCODE" }
            & $ctl attach-id $endpointId
            if ($LASTEXITCODE -ne 0) { throw "Could not restore Current EFX: $LASTEXITCODE" }
            Restart-Graph
            Wait-Endpoint $endpointId
            if ($before) {
                $restored = Get-MixSnapshot $endpointName
                Assert-GeometryPreserved $before $restored
            }
            $fx = Get-FxStatus $endpointId
            if (-not [string]::Equals($fx.Efx, $currentEfxClsid, [StringComparison]::OrdinalIgnoreCase) -or $fx.Sfx -ne '<absent>') {
                throw "Rollback FX state is not clean. EFX=$($fx.Efx) SFX=$($fx.Sfx)"
            }
        }

        if ($nativeRegistered) {
            Set-AudioServiceRunning $false
            try {
                $regsvr32 = Join-Path $env:WINDIR 'System32\regsvr32.exe'
                $unregister = Start-Process -FilePath $regsvr32 -ArgumentList @('/u', '/s', "`"$installedStreamApo`"") -Wait -PassThru
                if ($unregister.ExitCode -ne 0) { Write-Warning "Native stream APO unregister returned $($unregister.ExitCode)" }
            }
            finally { Set-AudioServiceRunning $true }
        }
    }
    catch {
        throw "Adaptive migration failed and rollback also failed: $($_.Exception.Message)"
    }

    # The endpoint EFX only runs Current on stereo. Never report a successful
    # rollback on a multichannel endpoint where that fallback would be transparent.
    if ($before -and $before.Channels -ne 2) {
        throw "Adaptive native SFX migration failed on a $($before.Channels)-channel endpoint; rollback is clean but cannot render Current on this endpoint. Original failure: $($failure.Exception.Message)"
    }

    Write-Host 'OMNIPHONY_WINDOWS_INSTALL_OK 1'
    Write-Host 'OMNIPHONY_INSTALL_STAGE stereo-current-rollback'
}
finally {
    if ($transcriptStarted) { try { Stop-Transcript | Out-Null } catch { } }
}
