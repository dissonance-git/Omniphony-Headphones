$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$programData = if ([string]::IsNullOrWhiteSpace($env:ProgramData)) { 'C:\ProgramData' } else { $env:ProgramData }
$stateRoot = Join-Path $programData 'Omniphony'
$currentPath = Join-Path $stateRoot 'current-enabled.txt'
$eqPresetPath = Join-Path $stateRoot 'eq-preset.txt'
$legacyEqPath = Join-Path $stateRoot 'personal-eq.txt'
$enhancementPath = Join-Path $stateRoot 'noire-x-enhancement.txt'
$restartAudioPath = Join-Path $PSScriptRoot 'Restart-OmniphonyAudio.ps1'
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

$notify = New-Object System.Windows.Forms.NotifyIcon
$notify.Icon = [System.Drawing.SystemIcons]::Application
$notify.Visible = $true

$menu = New-Object System.Windows.Forms.ContextMenuStrip
$statusItem = New-Object System.Windows.Forms.ToolStripMenuItem
$statusItem.Text = 'Omniphony'
$statusItem.Enabled = $false
[void]$menu.Items.Add($statusItem)

$currentItem = New-Object System.Windows.Forms.ToolStripMenuItem
$currentItem.Text = 'Enabled'
[void]$menu.Items.Add($currentItem)

$eqItem = New-Object System.Windows.Forms.ToolStripMenuItem
[void]$menu.Items.Add($eqItem)

$enhancementItem = New-Object System.Windows.Forms.ToolStripMenuItem
[void]$menu.Items.Add($enhancementItem)

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

    $currentItem.Checked = $current
    $currentItem.Text = 'Enabled'

    $eqItem.Checked = $eq
    $eqItem.Text = if ($eq) { 'Headphone EQ: Noire X' } else { 'Headphone EQ: Off' }

    $enhancementItem.Checked = $enhancement
    $enhancementItem.Text = if ($enhancement) { 'Noire X Enhancement: On' } else { 'Noire X Enhancement: Off' }

    $enabledText = if ($current) { 'Enabled' } else { 'Disabled' }
    $eqText = if ($eq) { 'EQ On' } else { 'EQ Off' }
    $enhanceText = if ($enhancement) { 'NX On' } else { 'NX Off' }
    $statusItem.Text = "Omniphony | $enabledText | $eqText | $enhanceText"
    $notify.Text = "Omniphony | $enabledText | $enhanceText"
}

function Toggle-Current {
    try {
        $previous = Get-CurrentEnabled
        $next = -not $previous
        Set-OnOffSetting $currentPath $next 'on'
        Update-TrayState
        # Enabled-vs-identity is chosen when the APO graph locks. A graph reset
        # makes the switch exact and gives identity zero Omniphony latency.
        if (-not (Restart-WindowsAudioService $false)) {
            Set-OnOffSetting $currentPath $previous 'on'
            Update-TrayState
            return
        }
        Update-TrayState
        $message = if ($next) {
            'Omniphony enabled.'
        } else {
            'Omniphony bypassed. Authored surround remains source-faithful.'
        }
        Show-TrayMessage $message
    } catch {
        Show-TrayMessage "Could not change Omniphony state: $($_.Exception.Message)"
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

$currentItem.Add_Click({ Toggle-Current })
$eqItem.Add_Click({ Toggle-Eq })
$enhancementItem.Add_Click({ Toggle-Enhancement })
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
