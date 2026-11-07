; Inno Setup script for bikey on Windows. Build with scripts/build-installer.ps1.
;
; Windows loads bikey.dll into every app that accepts text, including elevated
; ones, so it must live where only administrators can write. The install folder
; is therefore fixed: there is no page to change it.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
; The release workflow passes code-signed copies of the DLLs here.
#ifndef X64Dll
  #define X64Dll "..\target\release\bikey.dll"
#endif
#ifndef X86Dll
  #define X86Dll "..\target\i686-pc-windows-msvc\release\bikey.dll"
#endif

[Setup]
AppId={{B7E0C3A2-5D41-4F6E-9A87-3C2D1E0F4B65}
AppName=bikey
AppVersion={#AppVersion}
; Code signing (SignPath) checks the product name and version of the setup file.
VersionInfoVersion={#AppVersion}
VersionInfoProductName=bikey
VersionInfoProductVersion={#AppVersion}
VersionInfoProductTextVersion={#AppVersion}
VersionInfoDescription=bikey Setup
VersionInfoCompany=Bryan Le
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
SetupIconFile=..\platforms\windows\res\bikey.ico
WizardSmallImageFile=..\platforms\windows\res\wizard-58.bmp,..\platforms\windows\res\wizard-116.bmp
UninstallDisplayIcon={app}\x64\bikey.dll
UninstallDisplayName=bikey
OutputDir=..\target\installer
OutputBaseFilename=bikey-setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "defaultkeyboard"; Description: "Make bikey my default keyboard (it stays on after a restart)"

[Files]
Source: "default-keyboard.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#X64Dll}"; DestDir: "{app}\x64"; Flags: ignoreversion regserver 64bit uninsrestartdelete
Source: "{#X86Dll}"; DestDir: "{app}\x86"; Flags: ignoreversion regserver 32bit uninsrestartdelete
Source: "..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme

[Run]
; Keyboard settings are per-user, so this runs as the person who started setup.
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File ""{app}\default-keyboard.ps1"""; Flags: runasoriginaluser runhidden; Tasks: defaultkeyboard

[UninstallRun]
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File ""{app}\default-keyboard.ps1"" -Remove"; Flags: runhidden; RunOnceId: "RemoveKeyboard"

[Messages]
FinishedLabel=bikey is installed.%n%n1. Turn off UniKey, EVKey or OpenKey, including at startup: running one with bikey scrambles text.%n2. Fully quit and reopen apps that were already running.%n3. If bikey is not selected, press Win+Space and choose it.%n%nTắt UniKey, EVKey hoặc OpenKey, mở lại các ứng dụng đang chạy. Nếu bikey chưa được chọn, nhấn Win+Space và chọn bikey.

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
