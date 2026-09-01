#ifndef SourceRoot
  #error SourceRoot must point to the staged application directory
#endif
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef OutputDir
  #define OutputDir ".\output"
#endif

[Setup]
AppId={{715F8FA6-31A4-4C27-84B8-80306CD9B6F3}
AppName=Mosaic
AppVersion={#AppVersion}
AppVerName=Mosaic {#AppVersion}
AppPublisher=Mosaic contributors
AppPublisherURL=https://github.com/Quill-00/mosaic-desktop-automation
AppSupportURL=https://github.com/Quill-00/mosaic-desktop-automation/issues
AppUpdatesURL=https://github.com/Quill-00/mosaic-desktop-automation/releases
DefaultDirName={localappdata}\Programs\Mosaic
DefaultGroupName=Mosaic
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=Mosaic-Setup-{#AppVersion}
SetupIconFile=..\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\Mosaic.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=yes
CloseApplicationsFilter=Mosaic.exe
MinVersion=10.0

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "附加快捷方式:"; Flags: checkedonce
Name: "startup"; Description: "登录 Windows 后自动启动 Mosaic"; GroupDescription: "启动选项:"; Flags: unchecked

[Files]
Source: "{#SourceRoot}\Mosaic.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceRoot}\resources\cliproxyapi\cli-proxy-api.exe"; DestDir: "{app}\resources\cliproxyapi"; Flags: ignoreversion
Source: "{#SourceRoot}\resources\cliproxyapi\LICENSE"; DestDir: "{app}\resources\cliproxyapi"; Flags: ignoreversion
Source: "{#SourceRoot}\resources\cliproxyapi\config.example.yaml"; DestDir: "{app}\resources\cliproxyapi"; Flags: ignoreversion
Source: "{#SourceRoot}\resources\cliproxyapi\config.empty.yaml"; DestDir: "{app}\resources\cliproxyapi"; Flags: ignoreversion
Source: "{#SourceRoot}\resources\cliproxyapi\PROVENANCE.txt"; DestDir: "{app}\resources\cliproxyapi"; Flags: ignoreversion

[Icons]
Name: "{group}\Mosaic"; Filename: "{app}\Mosaic.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Mosaic"; Filename: "{app}\Mosaic.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Mosaic"; ValueData: """{app}\Mosaic.exe"""; Flags: uninsdeletevalue; Tasks: startup

[Run]
Filename: "{app}\Mosaic.exe"; Description: "启动 Mosaic"; Flags: nowait postinstall skipifsilent
Filename: "{app}\Mosaic.exe"; Flags: nowait runasoriginaluser; Check: IsAutoUpdate

[Code]
function HasCommandLineSwitch(const SwitchName: String): Boolean;
var
  Index: Integer;
begin
  Result := False;
  for Index := 1 to ParamCount do
  begin
    if CompareText(ParamStr(Index), SwitchName) = 0 then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

function IsAutoUpdate: Boolean;
begin
  Result := HasCommandLineSwitch('/AUTOUPDATE');
end;
