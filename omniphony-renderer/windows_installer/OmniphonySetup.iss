#ifndef PayloadDir
  #error PayloadDir must be supplied by the build workflow
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

#define MyAppName "Omniphony for Windows"
#define MyAppVersion "Current"
#define MyAppPublisher "Omniphony"

[Setup]
AppId={{6A6873B9-1199-4D6B-AC3E-9415E5BC6BB1}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
VersionInfoVersion=0.1.0.0
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription=Omniphony spatial headphone renderer and Windows audio integration
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion=0.1.0.0
DefaultDirName={autopf}\Omniphony
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
DisableWelcomePage=yes
DisableDirPage=yes
DisableReadyPage=yes
DisableFinishedPage=yes
ShowLanguageDialog=no
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
SetupLogging=yes
OutputDir={#OutputDir}
OutputBaseFilename=OmniphonySetup
UninstallDisplayName={#MyAppName}
CloseApplications=yes
RestartApplications=no

[InstallDelete]
Type: files; Name: "{app}\Omniphony.exe"
Type: files; Name: "{app}\PRODUCT-CONTEXT.md"
Type: filesandordirs; Name: "{app}\driver"
Type: filesandordirs; Name: "{app}\EndpointAPO"
Type: filesandordirs; Name: "{app}\support"

[Dirs]
Name: "{commonappdata}\Omniphony"; Permissions: users-modify

[Files]
; Runtime files are staged only for setup. The Windows installer establishes the
; proven endpoint baseline, then upgrades to the pre-mix native-surround SFX only
; after its own lifecycle smoke succeeds. Failure stops without claiming a state
; restoration that has not been read back and verified.
Source: "{#PayloadDir}\runtime\*"; DestDir: "{tmp}\OmniphonyAPOPayload"; Flags: ignoreversion recursesubdirs createallsubdirs deleteafterinstall
Source: "{#PayloadDir}\support\*"; DestDir: "{app}\support"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "endpoint_apo\Install-OmniphonyWindows.ps1"; DestDir: "{app}\support"; Flags: ignoreversion
Source: "endpoint_apo\Uninstall-OmniphonyWindows.ps1"; DestDir: "{app}\support"; Flags: ignoreversion
Source: "endpoint_apo\OmniphonyTray.ps1"; DestDir: "{app}\support"; Flags: ignoreversion
Source: "endpoint_apo\Restart-OmniphonyAudio.ps1"; DestDir: "{app}\support"; Flags: ignoreversion
Source: "{#PayloadDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userstartup}\Omniphony"; Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File ""{app}\support\OmniphonyTray.ps1"""; WorkingDir: "{app}\support"

[Run]
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File ""{app}\support\OmniphonyTray.ps1"""; WorkingDir: "{app}\support"; Flags: nowait runhidden runasoriginaluser

[UninstallRun]
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -Command ""New-Item -ItemType Directory -Force -Path '{commonappdata}\Omniphony' | Out-Null; Set-Content -LiteralPath '{commonappdata}\Omniphony\tray.stop' -Value stop -Encoding ASCII"""; Flags: runhidden waituntilterminated; RunOnceId: "OmniphonyTrayStop"
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File ""{app}\support\Uninstall-OmniphonyWindows.ps1"""; Flags: runhidden waituntilterminated; RunOnceId: "OmniphonyAudioCleanup"

[Code]
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
  TaskKill: String;
  TrayStop: String;
begin
  ForceDirectories(ExpandConstant('{commonappdata}\Omniphony'));
  TrayStop := ExpandConstant('{commonappdata}\Omniphony\tray.stop');
  SaveStringToFile(TrayStop, 'stop', False);

  TaskKill := ExpandConstant('{sys}\taskkill.exe');
  Exec(TaskKill, '/F /T /IM Omniphony.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);

  RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Omniphony');
  RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Spatial');

  NeedsRestart := False;
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
  PowerShell: String;
  Params: String;
begin
  if CurStep = ssPostInstall then
  begin
    PowerShell := ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe');

    { Establish the proven stereo endpoint baseline first, then swap to pre-mix
      SFX when the native-surround path validates on this machine. }
    Params := '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' +
      ExpandConstant('{app}\support\Install-OmniphonyWindows.ps1') +
      '" -PackageRoot "' + ExpandConstant('{tmp}\OmniphonyAPOPayload') +
      '" -AppRoot "' + ExpandConstant('{app}') + '" -AllowUnprotectedAudioDG';

    if (not Exec(PowerShell, Params, '', SW_HIDE, ewWaitUntilTerminated, ResultCode)) or
       (ResultCode <> 0) then
    begin
      RaiseException(
        'Omniphony could not validate attachment to the current Windows output, so setup stopped without claiming success. Diagnostic log: C:\ProgramData\Omniphony\install-last.log'
      );
    end;

    { The Spatial Sound provider remains an explicit development gate until
      registration, activation, selection, real-object ingress, and physical
      one-render egress are proven together. Setup must never select an
      unproven provider merely because its capability code compiled. }
  end;
end;