; Inno Setup Script for ZenOmen
; Generates ZenOmen-Setup-x64.exe

#define MyAppName "ZenOmen"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "ZenOmen Contributors"
#define MyAppURL "https://github.com/Omprakash-p06/zenomen"
#define MyAppExeName "zenomen.exe"
#define MyServiceExeName "zenomen-hw.exe"

#ifndef SourceDir
  #define SourceDir "..\..\..\target\release"
#endif

#ifndef ProjectRoot
  #define ProjectRoot "..\..\.."
#endif

#ifndef OutputDir
  #define OutputDir "..\..\..\target\installer"
#endif

[Setup]
AppId={{4E1A9D5B-3B9B-4D78-9B92-7E5A2A1A7D10}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile={#ProjectRoot}\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename=ZenOmen-Setup-x64
SetupIconFile={#ProjectRoot}\data\icons\zenomen.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
CloseApplications=force
UninstallDisplayIcon={app}\zenomen.ico
UninstallDisplayName={#MyAppName}
VersionInfoVersion={#MyAppVersion}.0
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription=ZenOmen Hardware Telemetry & Control for HP Laptops
VersionInfoCopyright=Copyright (C) 2026 {#MyAppPublisher}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Types]
Name: "full"; Description: "Full installation (GUI and Hardware Service)"
Name: "compact"; Description: "Compact installation (GUI only)"
Name: "custom"; Description: "Custom installation"; Flags: iscustom

[Components]
Name: "gui"; Description: "ZenOmen Graphical Interface"; Types: full compact custom; Flags: fixed
Name: "service"; Description: "ZenOmen Hardware Control Service (recommended for background fan control)"; Types: full custom

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "Start ZenOmen automatically on Windows logon (minimized to system tray)"; GroupDescription: "Startup options:"
Name: "addtopath"; Description: "Add ZenOmen to system PATH (recommended for CLI usage)"; GroupDescription: "System configuration:"

[Files]
Source: "{#SourceDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion; Components: gui
Source: "{#SourceDir}\{#MyServiceExeName}"; DestDir: "{app}"; Flags: ignoreversion; Components: service
Source: "{#ProjectRoot}\data\icons\zenomen.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectRoot}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectRoot}\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Icons]
Name: "{autoprograms}\{#MyAppName}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\zenomen.ico"; Components: gui
Name: "{autoprograms}\{#MyAppName}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\zenomen.ico"; Tasks: desktopicon; Components: gui

[Registry]
; Windows logon autostart
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#MyAppName}"; ValueData: """{app}\{#MyAppExeName}"" --minimized"; Flags: uninsdeletevalue; Tasks: autostart

[Run]
; Install and register Windows service if component selected
Filename: "{app}\{#MyServiceExeName}"; Parameters: "--install-service"; Flags: runhidden waituntilterminated; StatusMsg: "Installing and registering ZenOmen Hardware Service..."; Components: service
; Option to launch GUI after setup completes
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent; Components: gui

[UninstallRun]
; Gracefully stop and unregister Windows service before deleting files
Filename: "{app}\{#MyServiceExeName}"; Parameters: "--uninstall-service"; Flags: runhidden waituntilterminated; StatusMsg: "Stopping and unregistering ZenOmen Hardware Service..."; Components: service

[Code]
const
  EnvironmentKey = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';
  WM_SETTINGCHANGE = $001A;
  SMTO_ABORTIFHUNG = $0002;

function SendMessageTimeout(
  hWnd: HWND;
  Msg: Cardinal;
  wParam: LongInt;
  lParam: String;
  fuFlags: Cardinal;
  uTimeout: Cardinal;
  out lpdwResult: Cardinal
): LongInt; external 'SendMessageTimeoutW@user32.dll stdcall';

procedure BroadcastEnvironmentChange();
var
  DummyResult: Cardinal;
begin
  SendMessageTimeout($FFFF, WM_SETTINGCHANGE, 0, 'Environment', SMTO_ABORTIFHUNG, 5000, DummyResult);
end;

procedure AddAppToPath();
var
  CurrentPath, AppPath: string;
begin
  AppPath := ExpandConstant('{app}');
  if RegQueryStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', CurrentPath) then
  begin
    if Pos(';' + UpperCase(AppPath) + ';', ';' + UpperCase(CurrentPath) + ';') = 0 then
    begin
      if (Length(CurrentPath) > 0) and (CurrentPath[Length(CurrentPath)] <> ';') then
        CurrentPath := CurrentPath + ';';
      CurrentPath := CurrentPath + AppPath;
      if RegWriteStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', CurrentPath) then
        BroadcastEnvironmentChange();
    end;
  end;
end;

procedure RemoveAppFromPath();
var
  CurrentPath, AppPath, CleanedPath: string;
  P, L: Integer;
begin
  AppPath := ExpandConstant('{app}');
  if RegQueryStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', CurrentPath) then
  begin
    P := Pos(';' + UpperCase(AppPath) + ';', ';' + UpperCase(CurrentPath) + ';');
    if P > 0 then
    begin
      // Strip out the matched path cleanly
      CleanedPath := CurrentPath;
      L := Length(AppPath);
      if (P = 1) then
      begin
        Delete(CleanedPath, 1, L);
        if (Length(CleanedPath) > 0) and (CleanedPath[1] = ';') then
          Delete(CleanedPath, 1, 1);
      end
      else
      begin
        Delete(CleanedPath, P - 1, L + 1);
      end;
      if RegWriteStringValue(HKEY_LOCAL_MACHINE, EnvironmentKey, 'Path', CleanedPath) then
        BroadcastEnvironmentChange();
    end;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if IsTaskSelected('addtopath') then
      AddAppToPath();
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveAppFromPath();
  end;
end;
