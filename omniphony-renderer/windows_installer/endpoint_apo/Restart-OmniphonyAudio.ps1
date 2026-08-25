$ErrorActionPreference = 'Stop'

$timeout = [TimeSpan]::FromSeconds(15)

$audio = Get-Service -Name 'Audiosrv' -ErrorAction Stop
if ($audio.Status -ne 'Stopped') {
    Stop-Service -Name 'Audiosrv' -Force -ErrorAction Stop
    (Get-Service -Name 'Audiosrv' -ErrorAction Stop).WaitForStatus('Stopped', $timeout)
}

$builder = Get-Service -Name 'AudioEndpointBuilder' -ErrorAction Stop
if ($builder.Status -ne 'Running') {
    Start-Service -Name 'AudioEndpointBuilder' -ErrorAction Stop
    (Get-Service -Name 'AudioEndpointBuilder' -ErrorAction Stop).WaitForStatus('Running', $timeout)
}

Start-Service -Name 'Audiosrv' -ErrorAction Stop
(Get-Service -Name 'Audiosrv' -ErrorAction Stop).WaitForStatus('Running', $timeout)
