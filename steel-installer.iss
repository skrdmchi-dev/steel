; =========================================================================
; Steel (.steel) v1.0.0 — Official Inno Setup Script (steel-installer.iss)
; Compile with Inno Setup 6 (`ISCC.exe steel-installer.iss`)
; =========================================================================

[Setup]
AppId={{8D7F2E1A-4C9B-4E3A-9F2B-1A5C8E7D9012}
AppName=Steel Programming Language
AppVersion=1.0.0
AppVerName=Steel (.steel) v1.0.0
AppPublisher=Steel Language Team
DefaultDirName={userpf}\Steel
DefaultGroupName=Steel Programming Language
OutputDir=.
OutputBaseFilename=steel-setup-inno
WizardStyle=classic
Compression=lzma
SolidCompression=yes
ChangesEnvironment=yes
PrivilegesRequired=lowest

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Types]
Name: "full"; Description: "Full installation (Compiler, PATH, VS Code Extension & File Associations)"
Name: "compact"; Description: "Core Compiler CLI only"
Name: "custom"; Description: "Custom installation"; Flags: iscustom

[Components]
Name: "compiler"; Description: "Steel Multi-Target Compiler, VM & Test Runner (steel.exe)"; Types: full compact custom; Flags: fixed
Name: "vscode"; Description: "Steel VS Code Extension (Syntax Colors, Logo Icon & Hover Types)"; Types: full custom
Name: "examples"; Description: "Example .steel Programs & Visual Web Playground"; Types: full custom

[Tasks]
Name: "envpath"; Description: "Add Steel to your Windows PATH (run 'steel compile' in any terminal)"; GroupDescription: "Additional tasks:"
Name: "fileassoc"; Description: "Associate .steel files with Steel in VS Code"; GroupDescription: "Additional tasks:"

[Files]
Source: "steel.exe"; DestDir: "{app}\bin"; Components: compiler; Flags: ignoreversion
Source: "logo.png"; DestDir: "{app}"; Components: compiler; Flags: ignoreversion
Source: "logo.svg"; DestDir: "{app}"; Components: compiler; Flags: ignoreversion
Source: "steel-lang-vscode-1.0.0.vsix"; DestDir: "{app}"; Components: vscode; Flags: ignoreversion
Source: "playground.html"; DestDir: "{app}"; Components: examples; Flags: ignoreversion
Source: "examples\*"; DestDir: "{app}\examples"; Components: examples; Flags: ignoreversion recursesubdirs createallsubdirs

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}\bin"; Tasks: envpath; Check: NeedsAddPath(ExpandConstant('{app}\bin'))

[Run]
Filename: "code"; Parameters: "--install-extension ""{app}\steel-lang-vscode-1.0.0.vsix"" --force"; StatusMsg: "Installing Steel VS Code Extension..."; Components: vscode; Flags: runhidden skipifdoesntexist

[Code]
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath)
  then begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + UpperCase(Param) + ';', ';' + UpperCase(OrigPath) + ';') = 0;
end;
