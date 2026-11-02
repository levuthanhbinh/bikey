; Inno Setup script for bikey on Windows. Build with scripts/build-installer.ps1.
;
; Windows loads bikey.dll into every app that accepts text, including elevated
; ones, so it must live where only administrators can write. The install folder
; is therefore fixed: there is no page to change it.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{B7E0C3A2-5D41-4F6E-9A87-3C2D1E0F4B65}
AppName=bikey
AppVersion={#AppVersion}
AppPublisher=Bryan Le
AppPublisherURL=https://github.com/levuthanhbinh/bikey
AppSupportURL=https://github.com/levuthanhbinh/bikey/issues
DefaultDirName={autopf}\bikey
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Never close the user's apps: they keep the old copy until they next restart.
CloseApplications=no
RestartApplications=no
LicenseFile=..\LICENSE-MIT
SetupIconFile=..\platforms\windows\res\vi.ico
UninstallDisplayIcon={app}\x64\bikey.dll
UninstallDisplayName=bikey
OutputDir=..\target\installer
OutputBaseFilename=bikey-setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Files]
Source: "..\target\release\bikey.dll"; DestDir: "{app}\x64"; Flags: ignoreversion regserver 64bit uninsrestartdelete
Source: "..\target\i686-pc-windows-msvc\release\bikey.dll"; DestDir: "{app}\x86"; Flags: ignoreversion regserver 32bit uninsrestartdelete
Source: "..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Messages]
FinishedLabel=bikey is installed.%n%n1. Turn off UniKey, EVKey or OpenKey: running one with bikey scrambles text.%n2. Fully quit and reopen apps that were already running.%n3. Press Win+Space and choose bikey.%n%nTắt UniKey, EVKey hoặc OpenKey, mở lại các ứng dụng đang chạy, rồi nhấn Win+Space và chọn bikey.

[Code]
// A DLL in use by running apps cannot be replaced, but it can be renamed. They
// keep using the renamed copy until they restart, and no reboot is needed.
procedure RetireDll(const Dll, RegSvr: String);
var
  ResultCode: Integer;
begin
  if not FileExists(Dll) then
    Exit;
  Exec(RegSvr, '/s /u "' + Dll + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  RenameFile(Dll, Dll + '.' + GetDateTimeString('yyyymmddhhnnss', #0, #0) + '.old');
end;

// Delete retired copies, or at the next restart if an app still has one loaded.
procedure RemoveOldCopies(const Dir: String);
var
  Found: TFindRec;
begin
  if FindFirst(Dir + '\*.old', Found) then
  try
    repeat
      if not DeleteFile(Dir + '\' + Found.Name) then
        RestartReplace(Dir + '\' + Found.Name, '');
    until not FindNext(Found);
  finally
    FindClose(Found);
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  RetireDll(ExpandConstant('{app}\x64\bikey.dll'), ExpandConstant('{sys}\regsvr32.exe'));
  RetireDll(ExpandConstant('{app}\x86\bikey.dll'), ExpandConstant('{syswow64}\regsvr32.exe'));
  RemoveOldCopies(ExpandConstant('{app}\x64'));
  RemoveOldCopies(ExpandConstant('{app}\x86'));
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveOldCopies(ExpandConstant('{app}\x64'));
    RemoveOldCopies(ExpandConstant('{app}\x86'));
  end;
end;
