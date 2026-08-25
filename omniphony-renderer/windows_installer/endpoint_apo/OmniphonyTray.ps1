$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$programData = if ([string]::IsNullOrWhiteSpace($env:ProgramData)) { 'C:\ProgramData' } else { $env:ProgramData }
$stateRoot = Join-Path $programData 'Omniphony'
$currentPath = Join-Path $stateRoot 'current-enabled.txt'
$eqPresetPath = Join-Path $stateRoot 'eq-preset.txt'
$legacyEqPath = Join-Path $stateRoot 'personal-eq.txt'
$enhancementPath = Join-Path $stateRoot 'noire-x-enhancement.txt'
$outputTrimPath = Join-Path $stateRoot 'output-trim.txt'
$endpointBackupPath = Join-Path $stateRoot 'endpoint-backup.json'
$restartAudioPath = Join-Path $PSScriptRoot 'Restart-OmniphonyAudio.ps1'
$spatialEnablePath = Join-Path $PSScriptRoot 'Enable-OmniphonySpatialProvider.ps1'
$spatialDisablePath = Join-Path $PSScriptRoot 'Disable-OmniphonySpatialProvider.ps1'
$spatialConfigPath = 'HKLM:\SOFTWARE\Omniphony\SpatialProvider'
$stopPath = Join-Path $stateRoot 'tray.stop'

New-Item -ItemType Directory -Force -Path $stateRoot | Out-Null
Remove-Item -LiteralPath $stopPath -Force -ErrorAction SilentlyContinue

$createdNew = $false
$mutex = New-Object System.Threading.Mutex($true, 'Local\OmniphonyTray', [ref]$createdNew)
if (-not $createdNew) {
    $mutex.Dispose()
    exit 0
}

function Get-OnOffSetting([string]$Path, [bool]$DefaultOn = $true) {
    try {
        if (Test-Path -LiteralPath $Path) {
            $text = ([IO.File]::ReadAllText($Path)).Trim().ToLowerInvariant()
            if ($text -in @('0', '0db', 'off', 'false', 'disabled', 'none', 'flat')) { return $false }
            return $true
        }
    } catch { }
    return $DefaultOn
}

function Set-OnOffSetting([string]$Path, [bool]$Enabled, [string]$OnValue = 'on') {
    $value = if ($Enabled) { $OnValue } else { 'off' }
    [IO.File]::WriteAllText($Path, "$value`r`n", [Text.Encoding]::ASCII)
}

function Get-CurrentEnabled { return Get-OnOffSetting $currentPath $true }

function Get-EqEnabled {
    try {
        if (Test-Path -LiteralPath $eqPresetPath) {
            return Get-OnOffSetting $eqPresetPath $true
        }
        if (Test-Path -LiteralPath $legacyEqPath) {
            return Get-OnOffSetting $legacyEqPath $true
        }
    } catch { }
    return $true
}

function Get-EnhancementEnabled { return Get-OnOffSetting $enhancementPath $true }
function Get-OutputTrimEnabled { return Get-OnOffSetting $outputTrimPath $true }

function Get-SpatialProviderEnabled {
    try {
        if (-not (Test-Path -LiteralPath $spatialConfigPath)) { return $false }
        $config = Get-ItemProperty -LiteralPath $spatialConfigPath -ErrorAction Stop
        return ([int]$config.Enabled -eq 1)
    } catch { }
    return $false
}

function Get-InstalledEndpointId {
    try {
        if (-not (Test-Path -LiteralPath $endpointBackupPath -PathType Leaf)) { return '' }
        $state = ([IO.File]::ReadAllText($endpointBackupPath)) | ConvertFrom-Json
        return [string]$state.EndpointId
    } catch { }
    return ''
}

function Show-TrayMessage([string]$Text) {
    $notify.BalloonTipTitle = 'Omniphony'
    $notify.BalloonTipText = $Text
    $notify.ShowBalloonTip(2500)
}

function Invoke-ElevatedPowerShellScript([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required helper is missing: $Path"
    }
    $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $process = Start-Process -FilePath $powershell `
        -Verb RunAs `
        -ArgumentList @(
            '-NoProfile',
            '-NonInteractive',
            '-ExecutionPolicy', 'Bypass',
            '-WindowStyle', 'Hidden',
            '-File', "`"$Path`""
        ) `
        -Wait `
        -PassThru
    if ($process.ExitCode -ne 0) {
        throw "Elevated helper exited with code $($process.ExitCode)."
    }
}

function Restart-WindowsAudioService([bool]$ShowSuccess = $true) {
    try {
        if (-not (Test-Path -LiteralPath $restartAudioPath)) {
            throw "Restart helper is missing: $restartAudioPath"
        }
        Invoke-ElevatedPowerShellScript $restartAudioPath
        if ($ShowSuccess) { Show-TrayMessage 'Windows Audio service restarted.' }
        return $true
    } catch {
        Show-TrayMessage "Could not restart Windows Audio: $($_.Exception.Message)"
        return $false
    }
}

function Open-SpatialSoundSettings {
    try {
        $endpointId = Get-InstalledEndpointId
        if (-not [string]::IsNullOrWhiteSpace($endpointId)) {
            $escaped = [Uri]::EscapeDataString($endpointId)
            Start-Process -FilePath "ms-settings:sound-properties?endpointId=$escaped"
            return
        }
        Start-Process -FilePath 'ms-settings:sound'
    } catch {
        try {
            Start-Process -FilePath 'ms-settings:sound'
        } catch {
            Show-TrayMessage "Could not open Windows Sound settings: $($_.Exception.Message)"
        }
    }
}

$notify = New-Object System.Windows.Forms.NotifyIcon
$notify.Icon = [System.Drawing.SystemIcons]::Application
$notify.Visible = $true

$menu = New-Object System.Windows.Forms.ContextMenuStrip
$statusItem = New-Object System.Windows.Forms.ToolStripMenuItem
$statusItem.Text = 'Omniphony Controls'
$statusItem.Enabled = $false
[void]$menu.Items.Add($statusItem)

$currentItem = New-Object System.Windows.Forms.ToolStripMenuItem
[void]$menu.Items.Add($currentItem)

$eqItem = New-Object System.Windows.Forms.ToolStripMenuItem
[void]$menu.Items.Add($eqItem)

$enhancementItem = New-Object System.Windows.Forms.ToolStripMenuItem
[void]$menu.Items.Add($enhancementItem)

$outputTrimItem = New-Object System.Windows.Forms.ToolStripMenuItem
[void]$menu.Items.Add($outputTrimItem)

[void]$menu.Items.Add((New-Object System.Windows.Forms.ToolStripSeparator))

$spatialProviderItem = New-Object System.Windows.Forms.ToolStripMenuItem
[void]$menu.Items.Add($spatialProviderItem)

$spatialSettingsItem = New-Object System.Windows.Forms.ToolStripMenuItem
$spatialSettingsItem.Text = 'Open Windows Spatial sound settings...'
[void]$menu.Items.Add($spatialSettingsItem)

[void]$menu.Items.Add((New-Object System.Windows.Forms.ToolStripSeparator))

$restartAudioItem = New-Object System.Windows.Forms.ToolStripMenuItem
$restartAudioItem.Text = 'Restart Windows Audio Service'
[void]$menu.Items.Add($restartAudioItem)

[void]$menu.Items.Add((New-Object System.Windows.Forms.ToolStripSeparator))

$exitItem = New-Object System.Windows.Forms.ToolStripMenuItem
$exitItem.Text = 'Exit tray'
[void]$menu.Items.Add($exitItem)
$notify.ContextMenuStrip = $menu

function Update-TrayState {
    $current = Get-CurrentEnabled
    $eq = Get-EqEnabled
    $enhancement = Get-EnhancementEnabled
    $trim = Get-OutputTrimEnabled
    $spatial = Get-SpatialProviderEnabled
    $spatialHelpersPresent =
        (Test-Path -LiteralPath $spatialEnablePath -PathType Leaf) -and
        (Test-Path -LiteralPath $spatialDisablePath -PathType Leaf)

    $currentItem.Checked = $current
    $currentItem.Text = if ($current) { 'Stereo Current: On' } else { 'Stereo Current: Off (identity)' }

    $eqItem.Checked = $eq
    $eqItem.Text = if ($eq) { 'Headphone EQ: Noire X' } else { 'Headphone EQ: Off' }

    $enhancementItem.Checked = $enhancement
    $enhancementItem.Text = if ($enhancement) { 'Noire X Enhancement: On' } else { 'Noire X Enhancement: Off' }

    $outputTrimItem.Checked = $trim
    $outputTrimItem.Text = if ($trim) { 'Output trim: +1.5 dB' } else { 'Output trim: 0 dB' }

    $spatialProviderItem.Checked = $spatial
    $spatialProviderItem.Enabled = $spatialHelpersPresent
    if (-not $spatialHelpersPresent) {
        $spatialProviderItem.Text = 'Spatial Sound provider: Helpers missing'
    } elseif ($spatial) {
        $spatialProviderItem.Text = 'Spatial Sound provider: Enabled (17 static + 16 dynamic)'
    } else {
        $spatialProviderItem.Text = 'Spatial Sound provider: Disabled'
    }

    $currentText = if ($current) { 'Current On' } else { 'Current Off' }
    $eqText = if ($eq) { 'EQ On' } else { 'EQ Off' }
    $enhanceText = if ($enhancement) { 'NX On' } else { 'NX Off' }
    $trimText = if ($trim) { '+1.5dB' } else { '0dB' }
    $spatialText = if ($spatial) { 'Spatial On' } else { 'Spatial Off' }
    $statusItem.Text = "Omniphony Controls | $currentText | $spatialText | $eqText | $enhanceText | $trimText"
    $notify.Text = "Omniphony | $currentText | $spatialText | $enhanceText"
}

function Toggle-Current {
    try {
        $previous = Get-CurrentEnabled
        $next = -not $previous
        Set-OnOffSetting $currentPath $next 'on'
        Update-TrayState
        # Current-vs-identity is chosen when the APO graph locks. A graph reset
        # makes the switch exact and gives identity zero Omniphony latency.
        if (-not (Restart-WindowsAudioService $false)) {
            Set-OnOffSetting $currentPath $previous 'on'
            Update-TrayState
            return
        }
        Update-TrayState
        $message = if ($next) {
            'Stereo Current enabled.'
        } else {
            'Stereo Current bypassed. Authored surround remains source-faithful.'
        }
        Show-TrayMessage $message
    } catch {
        Show-TrayMessage "Could not change Stereo Current: $($_.Exception.Message)"
    }
}

function Toggle-Eq {
    try {
        Set-OnOffSetting $eqPresetPath (-not (Get-EqEnabled)) 'on'
        Update-TrayState
    } catch {
        Show-TrayMessage "Could not change the EQ setting: $($_.Exception.Message)"
    }
}

function Toggle-Enhancement {
    try {
        Set-OnOffSetting $enhancementPath (-not (Get-EnhancementEnabled)) 'on'
        Update-TrayState
    } catch {
        Show-TrayMessage "Could not change Noire X Enhancement: $($_.Exception.Message)"
    }
}

function Toggle-OutputTrim {
    try {
        Set-OnOffSetting $outputTrimPath (-not (Get-OutputTrimEnabled)) '+1.5'
        Update-TrayState
    } catch {
        Show-TrayMessage "Could not change output trim: $($_.Exception.Message)"
    }
}

function Toggle-SpatialProvider {
    try {
        $wasEnabled = Get-SpatialProviderEnabled
        if ($wasEnabled) {
            Invoke-ElevatedPowerShellScript $spatialDisablePath
        } else {
            Invoke-ElevatedPowerShellScript $spatialEnablePath
        }
        Update-TrayState

        if ($wasEnabled) {
            Show-TrayMessage 'Spatial Sound provider disabled and unregistered. Windows provider selection was not changed.'
        } else {
            Show-TrayMessage 'Spatial Sound provider enabled. Select Omniphony under Spatial sound in Windows settings.'
        }
    } catch {
        Update-TrayState
        Show-TrayMessage "Could not change Spatial Sound provider: $($_.Exception.Message)"
    }
}

$currentItem.Add_Click({ Toggle-Current })
$eqItem.Add_Click({ Toggle-Eq })
$enhancementItem.Add_Click({ Toggle-Enhancement })
$outputTrimItem.Add_Click({ Toggle-OutputTrim })
$spatialProviderItem.Add_Click({ Toggle-SpatialProvider })
$spatialSettingsItem.Add_Click({ Open-SpatialSoundSettings })
$restartAudioItem.Add_Click({ [void](Restart-WindowsAudioService $true) })

$exitItem.Add_Click({
    $notify.Visible = $false
    [System.Windows.Forms.Application]::Exit()
})

$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 500
$timer.Add_Tick({
    if ((Test-Path -LiteralPath $stopPath) -or -not (Test-Path -LiteralPath $PSCommandPath)) {
        $notify.Visible = $false
        [System.Windows.Forms.Application]::Exit()
        return
    }
    Update-TrayState
})

try {
    Update-TrayState
    $timer.Start()
    [System.Windows.Forms.Application]::Run()
} finally {
    $timer.Stop()
    $timer.Dispose()
    $notify.Visible = $false
    $notify.Dispose()
    $menu.Dispose()
    try { $mutex.ReleaseMutex() } catch { }
    $mutex.Dispose()
}
