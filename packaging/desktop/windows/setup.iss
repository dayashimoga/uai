; Inno Setup Script for Universal AI Assistant (Windows x86_64)
; Bundles Flutter Windows shell, Rust core dynamic library (aissistant_ffi.dll), and assets.

#define MyAppName "Universal AI Assistant"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "Universal AI Assistant Open Source Contributors"
#define MyAppURL "https://github.com/aissistant/aissistant"
#define MyAppExeName "aissistant.exe"
#define SourceDir "..\..\..\app\build\windows\x64\runner\Release"

[Setup]
AppId={{5E4A8C89-F2E3-4E1B-9473-841DF87A4F82}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile=..\..\..\LICENSE
OutputDir=..\..\..\dist
OutputBaseFilename=aissistant-windows-x64-setup-v{#MyAppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startup"; Description: "Automatically start Universal AI Assistant when signing in to Windows"; GroupDescription: "Startup:"; Flags: unchecked

[Files]
; Main Flutter runner executable
Source: "{#SourceDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
; Core Rust FFI engine
Source: "{#SourceDir}\aissistant_ffi.dll"; DestDir: "{app}"; Flags: ignoreversion
; Flutter engine runtime DLL
Source: "{#SourceDir}\flutter_windows.dll"; DestDir: "{app}"; Flags: ignoreversion
; Native plugins and data bundles
Source: "{#SourceDir}\*.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#SourceDir}\data\*"; DestDir: "{app}\data"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Configure optional autostart in CurrentUser Run registry key
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#MyAppName}"; ValueData: """{app}\{#MyAppExeName}"" --minimized"; Flags: uninsdeletevalue; Tasks: startup

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
