param(
    [string]$AppRoot = ''
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($AppRoot)) { $AppRoot = Join-Path $env:ProgramFiles 'Omniphony' }

$baselineUninstaller = Join-Path $here 'Uninstall-OmniphonyAPO.ps1'
$ctl = Join-Path $here 'OmniphonyApoCtl.exe'
$spatialDisable = Join-Path $here 'Disable-OmniphonySpatialProvider.ps1'
$stateRoot = Join-Path $env:ProgramData 'Omniphony'
$endpointBackupPath = Join-Path $stateRoot 'endpoint-backup.json'
$legacyStreamBackupPath = Join-Path $stateRoot 'stream-backup.json'
$installedNativeApo = Join-Path (Join-Path $AppRoot 'APO') 'OmniphonyStreamAPO.dll'
$nativeApoClsid = '{07D403D9-8A98-43EF-8C28-8651756D83BE}'
$spatialFormatGuid = '{4BD75423-A66C-4586-B782-1FCBBDF2AE74}'
$spatialProviderClsid = '{F3CDF827-20C4-405E-A430-8F739343FC89}'

function Set-AudioServiceRunning([bool]$Running) {
    $service = Get-Service -Name AudioSrv -ErrorAction Stop
    if ($Running -and $service.Status -ne 'Running') { Start-Service -Name AudioSrv }
    if ((-not $Running) -and $service.Status -ne 'Stopped') { Stop-Service -Name AudioSrv -Force }
}

function Restart-AudioGraph {
    Set-AudioServiceRunning $false
    Start-Sleep -Milliseconds 250
    Set-AudioServiceRunning $true
    Start-Sleep -Milliseconds 1000
}

function Remove-HklmTree([string]$Path) {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
        [Microsoft.Win32.RegistryHive]::LocalMachine,
        [Microsoft.Win32.RegistryView]::Registry64)
    try { $base.DeleteSubKeyTree($Path, $false) } finally { $base.Dispose() }
}

function Test-HklmTree([string]$Path) {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
        [Microsoft.Win32.RegistryHive]::LocalMachine,
        [Microsoft.Win32.RegistryView]::Registry64)
    try {
        $key = $base.OpenSubKey($Path, $false)
        if ($null -eq $key) { return $false }
        $key.Dispose()
        return $true
    } finally {
        $base.Dispose()
    }
}

# Never delete a Spatial Sound provider that Windows may still reference.
# The disable helper checks every active endpoint plus the saved Omniphony
# endpoint before touching the runtime gate or registration.
$spatialRuntimeKey = 'SOFTWARE\Omniphony\SpatialProvider'
$spatialEncoderKey = "SOFTWARE\Microsoft\Multimedia\Audio\Spatial\Encoder\$spatialFormatGuid"
$spatialComKey = "SOFTWARE\Classes\CLSID\$spatialProviderClsid"
$spatialOwnedStateExists =
    (Test-HklmTree $spatialRuntimeKey) -or
    (Test-HklmTree $spatialEncoderKey) -or
    (Test-HklmTree $spatialComKey)

if ($spatialOwnedStateExists) {
    if (-not (Test-Path -LiteralPath $spatialDisable -PathType Leaf)) {
        throw 'Omniphony Spatial Sound state exists but the guarded disable helper is missing. Uninstall stopped before deleting provider registration.'
    }
    & $spatialDisable
}

Remove-HklmTree $spatialRuntimeKey
Remove-HklmTree $spatialEncoderKey
Remove-HklmTree $spatialComKey

$endpointId = ''
if (Test-Path -LiteralPath $endpointBackupPath) {
    try {
        $endpointBackup = Get-Content -LiteralPath $endpointBackupPath -Raw | ConvertFrom-Json
        $endpointId = [string]$endpointBackup.EndpointId
    } catch {
        Write-Warning "Could not read endpoint backup before native-surround removal: $($_.Exception.Message)"
    }
}

# If the format-changing APO is active as the endpoint EFX, first move the
# endpoint back onto the proven stereo Current APO. This unloads the native DLL
# cleanly before its COM/APO registration and runtime file are removed.
if (-not [string]::IsNullOrWhiteSpace($endpointId) -and (Test-Path -LiteralPath $ctl)) {
    & $ctl cleanup-native-sfx-id $endpointId
    if ($LASTEXITCODE -ne 0) { Write-Warning "Legacy native-surround SFX cleanup returned $LASTEXITCODE" }

    & $ctl attach-id $endpointId
    if ($LASTEXITCODE -ne 0) {
        throw "Could not restore stereo Current before uninstalling native surround: $LASTEXITCODE"
    }
    Restart-AudioGraph
}

# Remove the additive native APO registration while AudioDG is down. Prefer its
# own unregister entry point when the DLL still exists, then defensively remove
# stale global keys so interrupted older installs cannot survive uninstall.
Set-AudioServiceRunning $false
try {
    if (Test-Path -LiteralPath $installedNativeApo) {
        $regsvr32 = Join-Path $env:WINDIR 'System32\regsvr32.exe'
        $quotedDll = "`"$installedNativeApo`""
        $process = Start-Process -FilePath $regsvr32 -ArgumentList @('/u', '/s', $quotedDll) -Wait -PassThru
        if ($process.ExitCode -ne 0) { Write-Warning "Native-surround APO unregister returned $($process.ExitCode)" }
    }
    Remove-HklmTree "SOFTWARE\Classes\AudioEngine\AudioProcessingObjects\$nativeApoClsid"
    Remove-HklmTree "SOFTWARE\Classes\CLSID\$nativeApoClsid"
} finally {
    Set-AudioServiceRunning $true
}

& $baselineUninstaller -AppRoot $AppRoot

if (Test-Path -LiteralPath $legacyStreamBackupPath) {
    Remove-Item -LiteralPath $legacyStreamBackupPath -Force -ErrorAction SilentlyContinue
}
Write-Host 'Omniphony Windows audio integration removed; optional Spatial Sound provider, native-surround, and stereo Current endpoint effects were removed safely.'
