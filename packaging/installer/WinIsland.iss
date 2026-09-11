#ifndef Channel
  #define Channel "stable"
#endif

#ifndef SourceDirectory
  #error "SourceDirectory must be provided."
#endif

#ifndef OutputDirectory
  #error "OutputDirectory must be provided."
#endif

#ifndef Version
  #error "Version must be provided."
#endif

#define AppName "WinIsland"
#define AppId "{{AA737034-4733-47BF-A8D4-3D7DB5B986D5}"
#define InstallDirectory "WinIsland"
#define IdentityPackageName "Eatgrapes.WinIsland"
#define IdentityPackageFile "Eatgrapes.WinIsland.msix"
#define IdentityCertificateFile "Eatgrapes.WinIsland.cer"

#if Channel == "nightly"
  #define OutputFile "WinIsland-Nightly-Setup"
#else
  #define OutputFile "WinIsland-Setup"
#endif

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#Version}
AppPublisher=Eatgrapes
DefaultDirName={localappdata}\{#InstallDirectory}
DisableProgramGroupPage=yes
OutputDir={#OutputDirectory}
OutputBaseFilename={#OutputFile}
SetupIconFile={#SourceDirectory}\resources\icon-dark.ico
UninstallDisplayIcon={app}\WinIsland.exe
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=force
RestartApplications=no

[Files]
Source: "{#SourceDirectory}\WinIsland.exe"; DestDir: "{app}"; Flags: ignoreversion restartreplace
Source: "{#SourceDirectory}\resources\icon-dark.png"; DestDir: "{app}\resources"; Flags: ignoreversion
Source: "{#SourceDirectory}\resources\icon-dark.ico"; DestDir: "{app}\resources"; Flags: ignoreversion
Source: "{#SourceDirectory}\resources\licenses\*"; DestDir: "{app}\resources\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#SourceDirectory}\identity\{#IdentityPackageFile}"; DestDir: "{app}\identity"; Flags: ignoreversion
Source: "{#SourceDirectory}\identity\{#IdentityCertificateFile}"; DestDir: "{app}\identity"; Flags: ignoreversion
Source: "{#SourceDirectory}\identity\Install-Identity.ps1"; DestDir: "{app}\identity"; Flags: ignoreversion
Source: "{#SourceDirectory}\identity\Remove-Identity.ps1"; DestDir: "{app}\identity"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\WinIsland.exe"

[Run]
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File ""{app}\identity\Install-Identity.ps1"" -PackagePath ""{app}\identity\{#IdentityPackageFile}"" -CertificatePath ""{app}\identity\{#IdentityCertificateFile}"" -ExternalLocation ""{app}"" -PackageName ""{#IdentityPackageName}"""; Flags: runhidden waituntilterminated
Filename: "{app}\WinIsland.exe"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File ""{app}\identity\Remove-Identity.ps1"" -PackageName ""{#IdentityPackageName}"""; Flags: runhidden waituntilterminated; RunOnceId: "Remove{#IdentityPackageName}"

#if Channel == "nightly"
[Code]
const
  LegacyNightlyUninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{B835C9EA-88D3-4CF3-9837-324EEDC65BD0}_is1';

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  UninstallCommand: String;
  ResultCode: Integer;
begin
  Result := '';
  if not RegQueryStringValue(HKEY_CURRENT_USER_64, LegacyNightlyUninstallKey,
    'QuietUninstallString', UninstallCommand) then
  begin
    RegQueryStringValue(HKEY_CURRENT_USER_64, LegacyNightlyUninstallKey,
      'UninstallString', UninstallCommand);
  end;

  if UninstallCommand = '' then
    exit;

  if not Exec('>', UninstallCommand + ' /VERYSILENT /SUPPRESSMSGBOXES /NORESTART',
    '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
  begin
    Result := 'The previous WinIsland Nightly installation could not be removed: ' +
      SysErrorMessage(ResultCode);
  end
  else if ResultCode <> 0 then
  begin
    Result := 'The previous WinIsland Nightly installation could not be removed (exit code ' +
      IntToStr(ResultCode) + ').';
  end;
end;
#endif
