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
