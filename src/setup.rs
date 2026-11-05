#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::env;
use std::fs;
use std::io::{self, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const PKG_JSON: &str = include_str!("../vscode-steel/package.json");
const LANG_CFG: &str = include_str!("../vscode-steel/language-configuration.json");
const TM_GRAMMAR: &str = include_str!("../vscode-steel/syntaxes/steel.tmLanguage.json");
const SNIPPETS: &str = include_str!("../vscode-steel/snippets/steel.json");
const EXT_JS: &str = include_str!("../vscode-steel/extension.js");
const EMBEDDED_LOGO_PNG: &[u8] = include_bytes!("../logo.png");
const EMBEDDED_LOGO_SVG: &[u8] = include_bytes!("../logo.svg");

const VSIX_MANIFEST: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011" xmlns:d="http://schemas.microsoft.com/developer/vsx-schema-design/2011">
  <Metadata>
    <Identity Language="en-US" Id="steel-lang-vscode" Version="1.0.0" Publisher="steel-lang" />
    <DisplayName>Steel Language (.steel)</DisplayName>
    <Description xml:space="preserve">Official VS Code syntax highlighting, inferred type hover intelligence, code snippets, and one-click multi-target compilation (--visual, C, Rust, Python, Go, JS, Lua) for the Steel programming language.</Description>
    <Tags>steel,compiler,programming-language</Tags>
    <Categories>Programming Languages</Categories>
    <GalleryFlags>Public</GalleryFlags>
    <Icon>extension/logo.png</Icon>
    <Properties>
      <Property Id="Microsoft.VisualStudio.Code.Engine" Value="^1.75.0" />
      <Property Id="Microsoft.VisualStudio.Code.ExtensionDependencies" Value="" />
      <Property Id="Microsoft.VisualStudio.Code.ExtensionPack" Value="" />
      <Property Id="Microsoft.VisualStudio.Code.ExtensionKind" Value="workspace" />
      <Property Id="Microsoft.VisualStudio.Services.Links.Source" Value="" />
    </Properties>
  </Metadata>
  <Installation>
    <InstallationTarget Id="Microsoft.VisualStudio.Code"/>
  </Installation>
  <Dependencies/>
  <Assets>
    <Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true" />
    <Asset Type="Microsoft.VisualStudio.Services.Icons.Default" Path="extension/logo.png" Addressable="true" />
  </Assets>
</PackageManifest>"#;

const CONTENT_TYPES_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension=".json" ContentType="application/json"/>
  <Default Extension=".js" ContentType="application/javascript"/>
  <Default Extension=".png" ContentType="image/png"/>
  <Default Extension=".svg" ContentType="image/svg+xml"/>
  <Default Extension=".ico" ContentType="image/x-icon"/>
  <Default Extension=".vsixmanifest" ContentType="text/xml"/>
</Types>"#;

struct SimpleZip {
    buf: Vec<u8>,
    entries: Vec<(String, u32, u32, u32)>,
}

impl SimpleZip {
    fn new() -> Self {
        Self {
            buf: Vec::new(),
            entries: Vec::new(),
        }
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                if (crc & 1) != 0 {
                    crc = (crc >> 1) ^ 0xEDB8_8320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }

    fn add_file(&mut self, name: &str, data: &[u8]) {
        let offset = self.buf.len() as u32;
        let crc = Self::crc32(data);
        let size = data.len() as u32;
        let name_bytes = name.as_bytes();

        self.buf.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        self.buf.extend_from_slice(&20u16.to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf.extend_from_slice(&crc.to_le_bytes());
        self.buf.extend_from_slice(&size.to_le_bytes());
        self.buf.extend_from_slice(&size.to_le_bytes());
        self.buf
            .extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf.extend_from_slice(name_bytes);
        self.buf.extend_from_slice(data);

        self.entries
            .push((name.to_string(), crc, size, offset));
    }

    fn finish(mut self) -> Vec<u8> {
        let cd_start = self.buf.len() as u32;
        for (name, crc, size, offset) in &self.entries {
            let name_bytes = name.as_bytes();
            self.buf.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            self.buf.extend_from_slice(&20u16.to_le_bytes());
            self.buf.extend_from_slice(&20u16.to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes());
            self.buf.extend_from_slice(&crc.to_le_bytes());
            self.buf.extend_from_slice(&size.to_le_bytes());
            self.buf.extend_from_slice(&size.to_le_bytes());
            self.buf
                .extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes());
            self.buf.extend_from_slice(&0u16.to_le_bytes());
            self.buf.extend_from_slice(&0u32.to_le_bytes());
            self.buf.extend_from_slice(&offset.to_le_bytes());
            self.buf.extend_from_slice(name_bytes);
        }
        let cd_size = (self.buf.len() as u32) - cd_start;
        let count = self.entries.len() as u16;

        self.buf.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());
        self.buf.extend_from_slice(&count.to_le_bytes());
        self.buf.extend_from_slice(&count.to_le_bytes());
        self.buf.extend_from_slice(&cd_size.to_le_bytes());
        self.buf.extend_from_slice(&cd_start.to_le_bytes());
        self.buf.extend_from_slice(&0u16.to_le_bytes());

        self.buf
    }
}

pub fn ensure_embedded_logo_file(steel_dir: &Path) -> PathBuf {
    let logo_dest = steel_dir.join("logo.png");
    let svg_dest = steel_dir.join("logo.svg");
    let _ = fs::write(&logo_dest, EMBEDDED_LOGO_PNG);
    let _ = fs::write(&svg_dest, EMBEDDED_LOGO_SVG);
    logo_dest
}

pub fn build_vsix_file(out_path: &Path, custom_logo: Option<&Path>) -> Result<(), String> {
    let mut zip = SimpleZip::new();
    zip.add_file("[Content_Types].xml", CONTENT_TYPES_XML.as_bytes());
    zip.add_file("extension.vsixmanifest", VSIX_MANIFEST.as_bytes());

    let logo_bytes: Vec<u8> = custom_logo
        .and_then(|p| fs::read(p).ok())
        .unwrap_or_else(|| EMBEDDED_LOGO_PNG.to_vec());

    zip.add_file("extension/logo.png", &logo_bytes);
    zip.add_file("extension/logo.svg", EMBEDDED_LOGO_SVG);
    zip.add_file("extension/package.json", PKG_JSON.as_bytes());
    zip.add_file(
        "extension/language-configuration.json",
        LANG_CFG.as_bytes(),
    );
    zip.add_file(
        "extension/syntaxes/steel.tmLanguage.json",
        TM_GRAMMAR.as_bytes(),
    );
    zip.add_file("extension/snippets/steel.json", SNIPPETS.as_bytes());
    zip.add_file("extension/extension.js", EXT_JS.as_bytes());

    let bytes = zip.finish();
    fs::write(out_path, bytes).map_err(|e| format!("Failed to write VSIX: {}", e))
}

fn find_source_steel_exe() -> Option<PathBuf> {
    if let Ok(cur_exe) = env::current_exe() {
        if let Some(dir) = cur_exe.parent() {
            let candidate = dir.join("steel.exe");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    let release_candidate = PathBuf::from("target/release/steel.exe");
    if release_candidate.exists() {
        return Some(release_candidate);
    }
    let debug_candidate = PathBuf::from("target/debug/steel.exe");
    if debug_candidate.exists() {
        return Some(debug_candidate);
    }
    None
}

fn register_in_extensions_json(base_ext_dir: &Path, ext_folder: &Path) {
    let json_path = base_ext_dir.join("extensions.json");
    let loc_path = ext_folder.to_string_lossy().replace('\\', "/");
    let loc_uri = format!(
        "file:///{}",
        loc_path.trim_start_matches('/').replace(':', "%3A")
    );
    let entry = format!(
        r#"{{"identifier":{{"id":"steel-lang.steel-lang-vscode"}},"version":"1.0.0","location":{{"$mid":1,"fsPath":"{}","external":"{}","path":"/{}","scheme":"file"}},"relativeLocation":"steel-lang.steel-lang-vscode-1.0.0","metadata":{{"installedTimestamp":1732195000000,"source":"vsix"}}}}"#,
        ext_folder.to_string_lossy().replace('\\', "\\\\"),
        loc_uri,
        loc_path
    );

    if let Ok(existing) = fs::read_to_string(&json_path) {
        if existing.contains("steel-lang.steel-lang-vscode") {
            return;
        }
        let trimmed = existing.trim();
        if trimmed == "[]" {
            let _ = fs::write(&json_path, format!("[{}]", entry));
        } else if let Some(stripped) = trimmed.strip_suffix(']') {
            let _ = fs::write(&json_path, format!("{},{}]", stripped, entry));
        }
    } else {
        let _ = fs::write(&json_path, format!("[{}]", entry));
    }
}

fn configure_ide_settings(settings_path: &Path) {
    if let Some(parent) = settings_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(existing) = fs::read_to_string(settings_path) {
        if existing.contains("\"*.steel\"") {
            return;
        }
        let trimmed = existing.trim();
        if trimmed == "{}" {
            let _ = fs::write(
                settings_path,
                "{\n  \"files.associations\": {\n    \"*.steel\": \"steel\"\n  }\n}\n",
            );
        } else if let Some(stripped) = trimmed.strip_suffix('}') {
            let comma = if stripped.trim_end().ends_with('{') {
                ""
            } else {
                ","
            };
            let _ = fs::write(
                settings_path,
                format!(
                    "{}{}\n  \"files.associations\": {{ \"*.steel\": \"steel\" }}\n}}\n",
                    stripped, comma
                ),
            );
        }
    } else {
        let _ = fs::write(
            settings_path,
            "{\n  \"files.associations\": {\n    \"*.steel\": \"steel\"\n  }\n}\n",
        );
    }
}

fn install_vscode_extension_full(home_dir: &Path, vsix_path: &Path, logo_path: &Path) {
    let ext_roots = [
        (home_dir.join(".vscode").join("extensions"), "Visual Studio Code"),
        (
            home_dir.join(".vscode-shared").join("extensions"),
            "VS Code Shared",
        ),
        (home_dir.join(".cursor").join("extensions"), "Cursor IDE"),
    ];

    for (base_ext_dir, ide_name) in &ext_roots {
        let dest = base_ext_dir.join("steel-lang.steel-lang-vscode-1.0.0");
        let syntaxes_dir = dest.join("syntaxes");
        let snippets_dir = dest.join("snippets");

        if fs::create_dir_all(&syntaxes_dir).is_ok() && fs::create_dir_all(&snippets_dir).is_ok() {
            let _ = fs::copy(logo_path, dest.join("logo.png"));
            let _ = fs::write(dest.join("logo.svg"), EMBEDDED_LOGO_SVG);
            let _ = fs::write(dest.join("package.json"), PKG_JSON);
            let _ = fs::write(dest.join("language-configuration.json"), LANG_CFG);
            let _ = fs::write(syntaxes_dir.join("steel.tmLanguage.json"), TM_GRAMMAR);
            let _ = fs::write(snippets_dir.join("steel.json"), SNIPPETS);
            let _ = fs::write(dest.join("extension.js"), EXT_JS);
            register_in_extensions_json(base_ext_dir, &dest);
            println!(
                "  \x1b[1;32m[OK]\x1b[0m Registered Steel Extension + Logo in \x1b[1m{}\x1b[0m",
                ide_name
            );
        }
    }

    let vsix_str = vsix_path.to_string_lossy().to_string();
    let code_cmd = home_dir
        .join("AppData")
        .join("Local")
        .join("Programs")
        .join("Microsoft VS Code")
        .join("bin")
        .join("code.cmd");
    if code_cmd.exists() {
        let status = Command::new(&code_cmd)
            .creation_flags(CREATE_NO_WINDOW)
            .args(["--install-extension", &vsix_str, "--force"])
            .status();
        if matches!(status, Ok(s) if s.success()) {
            println!("  \x1b[1;32m[OK]\x1b[0m Installed VSIX via official VS Code CLI (`code --install-extension`)");
        }
    }
}

fn associate_files(home_dir: &Path) {
    let roaming = home_dir.join("AppData").join("Roaming");
    for app in ["Code", "Cursor"] {
        let settings = roaming.join(app).join("User").join("settings.json");
        if settings.parent().map(|p| p.exists()).unwrap_or(false) {
            configure_ide_settings(&settings);
            println!(
                "  \x1b[1;32m[OK]\x1b[0m Associated `*.steel` files in {} User settings.json",
                app
            );
        }
    }
}

/// Classic Inno Setup WizardStyle=classic GUI!
/// - Standard Windows system gray dialog body (#F0F0F0)
/// - Crisp white top banner header (or white left sidebar on Welcome page) with your Steel 'S' logo
/// - Classic 3D etched horizontal separator lines
/// - Standard native Windows buttons (< Back, Next >, Install, Cancel)
fn run_classic_inno_setup_wizard(
    default_install_dir: &str,
    logo_path: &Path,
) -> Option<(String, bool, bool, bool, bool)> {
    let init_logo = logo_path.to_string_lossy().replace('\'', "''");
    let init_dir = default_install_dir.replace('\'', "''");

    let ps_wizard = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()

$script:logoPath = '{init_logo}'
$script:installDir = '{init_dir}'
$script:page = 1

$form = New-Object System.Windows.Forms.Form
$form.Text = "Setup - Steel Programming Language"
$form.ClientSize = New-Object System.Drawing.Size(500, 362)
$form.StartPosition = "CenterScreen"
$form.FormBorderStyle = "FixedDialog"
$form.MaximizeBox = $false
$form.MinimizeBox = $false
$form.BackColor = [System.Drawing.SystemColors]::Control
$form.ForeColor = [System.Drawing.SystemColors]::ControlText
$form.Font = New-Object System.Drawing.Font("Segoe UI", 9)

# Load Steel Logo Image
$steelLogoImg = $null
if ($script:logoPath -and (Test-Path $script:logoPath)) {{
    try {{
        $steelLogoImg = [System.Drawing.Image]::FromFile($script:logoPath)
        $bmpIcon = New-Object System.Drawing.Bitmap($steelLogoImg, 32, 32)
        $form.Icon = [System.Drawing.Icon]::FromHandle($bmpIcon.GetHicon())
    }} catch {{}}
}}

# =========================================================================
# TOP WHITE HEADER BANNER (Shown on Pages 2 & 3 - Classic Inno Setup Style)
# =========================================================================
$topHeader = New-Object System.Windows.Forms.Panel
$topHeader.Location = New-Object System.Drawing.Point(0, 0)
$topHeader.Size = New-Object System.Drawing.Size(500, 60)
$topHeader.BackColor = [System.Drawing.Color]::White
$topHeader.Visible = $false
$form.Controls.Add($topHeader)

$headerTitle = New-Object System.Windows.Forms.Label
$headerTitle.Location = New-Object System.Drawing.Point(16, 10)
$headerTitle.Size = New-Object System.Drawing.Size(410, 20)
$headerTitle.Font = New-Object System.Drawing.Font("Segoe UI", 9, [System.Drawing.FontStyle]::Bold)
$topHeader.Controls.Add($headerTitle)

$headerSub = New-Object System.Windows.Forms.Label
$headerSub.Location = New-Object System.Drawing.Point(30, 30)
$headerSub.Size = New-Object System.Drawing.Size(400, 24)
$headerSub.Font = New-Object System.Drawing.Font("Segoe UI", 9)
$topHeader.Controls.Add($headerSub)

$headerSmallLogo = New-Object System.Windows.Forms.PictureBox
$headerSmallLogo.Location = New-Object System.Drawing.Point(442, 8)
$headerSmallLogo.Size = New-Object System.Drawing.Size(44, 44)
$headerSmallLogo.SizeMode = "Zoom"
$headerSmallLogo.Image = $steelLogoImg
$topHeader.Controls.Add($headerSmallLogo)

$topLine = New-Object System.Windows.Forms.Label
$topLine.BorderStyle = [System.Windows.Forms.BorderStyle]::Fixed3D
$topLine.Location = New-Object System.Drawing.Point(0, 60)
$topLine.Size = New-Object System.Drawing.Size(500, 2)
$topLine.Visible = $false
$form.Controls.Add($topLine)

# =========================================================================
# PAGE 1: WELCOME PAGE (Classic White Left Sidebar + White Content Area)
# =========================================================================
$p1 = New-Object System.Windows.Forms.Panel
$p1.Location = New-Object System.Drawing.Point(0, 0)
$p1.Size = New-Object System.Drawing.Size(500, 312)
$p1.BackColor = [System.Drawing.Color]::White
$form.Controls.Add($p1)

$p1LeftBanner = New-Object System.Windows.Forms.Panel
$p1LeftBanner.Location = New-Object System.Drawing.Point(0, 0)
$p1LeftBanner.Size = New-Object System.Drawing.Size(164, 312)
$p1LeftBanner.BackColor = [System.Drawing.Color]::FromArgb(242, 244, 247)
$p1.Controls.Add($p1LeftBanner)

$p1BigLogo = New-Object System.Windows.Forms.PictureBox
$p1BigLogo.Location = New-Object System.Drawing.Point(22, 36)
$p1BigLogo.Size = New-Object System.Drawing.Size(120, 120)
$p1BigLogo.SizeMode = "Zoom"
$p1BigLogo.Image = $steelLogoImg
$p1LeftBanner.Controls.Add($p1BigLogo)

$p1SideText = New-Object System.Windows.Forms.Label
$p1SideText.Location = New-Object System.Drawing.Point(16, 170)
$p1SideText.Size = New-Object System.Drawing.Size(132, 60)
$p1SideText.TextAlign = "TopCenter"
$p1SideText.Font = New-Object System.Drawing.Font("Segoe UI", 8.5, [System.Drawing.FontStyle]::Bold)
$p1SideText.ForeColor = [System.Drawing.Color]::FromArgb(70, 75, 85)
$p1SideText.Text = "STEEL (.steel)`r`nVersion 1.0.0"
$p1LeftBanner.Controls.Add($p1SideText)

$p1VertLine = New-Object System.Windows.Forms.Label
$p1VertLine.BorderStyle = [System.Windows.Forms.BorderStyle]::Fixed3D
$p1VertLine.Location = New-Object System.Drawing.Point(164, 0)
$p1VertLine.Size = New-Object System.Drawing.Size(2, 312)
$p1.Controls.Add($p1VertLine)

$p1WelcomeTitle = New-Object System.Windows.Forms.Label
$p1WelcomeTitle.Location = New-Object System.Drawing.Point(184, 20)
$p1WelcomeTitle.Size = New-Object System.Drawing.Size(296, 48)
$p1WelcomeTitle.Font = New-Object System.Drawing.Font("Segoe UI", 12, [System.Drawing.FontStyle]::Bold)
$p1WelcomeTitle.Text = "Welcome to the Steel Programming Language Setup Wizard"
$p1.Controls.Add($p1WelcomeTitle)

$p1WelcomeBody = New-Object System.Windows.Forms.Label
$p1WelcomeBody.Location = New-Object System.Drawing.Point(184, 80)
$p1WelcomeBody.Size = New-Object System.Drawing.Size(296, 210)
$p1WelcomeBody.Text = "This will install Steel (.steel) version 1.0.0 on your computer.`r`n`r`nSteel includes:`r`n  • Multi-Target Compiler (C11, Rust, Python, Go, JS, Lua)`r`n  • Live 5-Stage Visual Morphing (--visual)`r`n  • Native Interpreter, Stack VM & Test Runner`r`n  • Official VS Code Extension with Syntax Highlighting`r`n`r`nIt is recommended that you close all other applications before continuing.`r`n`r`nClick Next to continue, or Cancel to exit Setup."
$p1.Controls.Add($p1WelcomeBody)

# =========================================================================
# PAGE 2: SELECT DESTINATION LOCATION & COMPONENTS (Classic System Gray)
# =========================================================================
$p2 = New-Object System.Windows.Forms.Panel
$p2.Location = New-Object System.Drawing.Point(0, 62)
$p2.Size = New-Object System.Drawing.Size(500, 250)
$p2.Visible = $false
$form.Controls.Add($p2)

$p2Desc = New-Object System.Windows.Forms.Label
$p2Desc.Location = New-Object System.Drawing.Point(24, 14)
$p2Desc.Size = New-Object System.Drawing.Size(450, 20)
$p2Desc.Text = "Setup will install Steel into the following folder:"
$p2.Controls.Add($p2Desc)

$dirBox = New-Object System.Windows.Forms.TextBox
$dirBox.Location = New-Object System.Drawing.Point(24, 38)
$dirBox.Size = New-Object System.Drawing.Size(362, 23)
$dirBox.Text = $script:installDir
$p2.Controls.Add($dirBox)

$btnBrowse = New-Object System.Windows.Forms.Button
$btnBrowse.Text = "Browse..."
$btnBrowse.Location = New-Object System.Drawing.Point(394, 37)
$btnBrowse.Size = New-Object System.Drawing.Size(80, 25)
$btnBrowse.UseVisualStyleBackColor = $true
$btnBrowse.Add_Click({{
    $fbd = New-Object System.Windows.Forms.FolderBrowserDialog
    $fbd.SelectedPath = $dirBox.Text
    if ($fbd.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{
        $dirBox.Text = $fbd.SelectedPath
    }}
}})
$p2.Controls.Add($btnBrowse)

$compGroup = New-Object System.Windows.Forms.GroupBox
$compGroup.Text = "Select Components && Additional Tasks"
$compGroup.Location = New-Object System.Drawing.Point(24, 74)
$compGroup.Size = New-Object System.Drawing.Size(450, 158)
$p2.Controls.Add($compGroup)

$cbCli = New-Object System.Windows.Forms.CheckBox
$cbCli.Text = "Steel Compiler, VM, Test Runner && Formatter (steel.exe)"
$cbCli.Checked = $true
$cbCli.Location = New-Object System.Drawing.Point(16, 26)
$cbCli.Size = New-Object System.Drawing.Size(420, 22)
$compGroup.Controls.Add($cbCli)

$cbPath = New-Object System.Windows.Forms.CheckBox
$cbPath.Text = "Add Steel to Windows PATH (use 'steel compile' in any terminal)"
$cbPath.Checked = $true
$cbPath.Location = New-Object System.Drawing.Point(16, 56)
$cbPath.Size = New-Object System.Drawing.Size(420, 22)
$compGroup.Controls.Add($cbPath)

$cbVsCode = New-Object System.Windows.Forms.CheckBox
$cbVsCode.Text = "Install Steel VS Code Extension (Syntax Colors, Logo && Hover Types)"
$cbVsCode.Checked = $true
$cbVsCode.Location = New-Object System.Drawing.Point(16, 86)
$cbVsCode.Size = New-Object System.Drawing.Size(425, 22)
$compGroup.Controls.Add($cbVsCode)

$cbAssoc = New-Object System.Windows.Forms.CheckBox
$cbAssoc.Text = "Associate *.steel files with Steel in VS Code Settings"
$cbAssoc.Checked = $true
$cbAssoc.Location = New-Object System.Drawing.Point(16, 116)
$cbAssoc.Size = New-Object System.Drawing.Size(420, 22)
$compGroup.Controls.Add($cbAssoc)

# =========================================================================
# PAGE 3: READY TO INSTALL (Classic Inno Setup Summary TextBox)
# =========================================================================
$p3 = New-Object System.Windows.Forms.Panel
$p3.Location = New-Object System.Drawing.Point(0, 62)
$p3.Size = New-Object System.Drawing.Size(500, 250)
$p3.Visible = $false
$form.Controls.Add($p3)

$p3Desc = New-Object System.Windows.Forms.Label
$p3Desc.Location = New-Object System.Drawing.Point(24, 14)
$p3Desc.Size = New-Object System.Drawing.Size(450, 34)
$p3Desc.Text = "Click Install to continue with the installation, or click Back if you want to review or change any settings."
$p3.Controls.Add($p3Desc)

$summaryBox = New-Object System.Windows.Forms.TextBox
$summaryBox.Location = New-Object System.Drawing.Point(24, 52)
$summaryBox.Size = New-Object System.Drawing.Size(450, 180)
$summaryBox.Multiline = $true
$summaryBox.ReadOnly = $true
$summaryBox.ScrollBars = "Vertical"
$summaryBox.BackColor = [System.Drawing.SystemColors]::ControlLightLight
$summaryBox.Font = New-Object System.Drawing.Font("Consolas", 9)
$p3.Controls.Add($summaryBox)

# =========================================================================
# BOTTOM SEPARATOR & STANDARD WINDOWS BUTTONS (< Back | Next > | Cancel)
# =========================================================================
$bottomLine = New-Object System.Windows.Forms.Label
$bottomLine.BorderStyle = [System.Windows.Forms.BorderStyle]::Fixed3D
$bottomLine.Location = New-Object System.Drawing.Point(0, 312)
$bottomLine.Size = New-Object System.Drawing.Size(500, 2)
$form.Controls.Add($bottomLine)

$btnBack = New-Object System.Windows.Forms.Button
$btnBack.Text = "< &Back"
$btnBack.Location = New-Object System.Drawing.Point(242, 325)
$btnBack.Size = New-Object System.Drawing.Size(75, 24)
$btnBack.UseVisualStyleBackColor = $true
$btnBack.Enabled = $false
$form.Controls.Add($btnBack)

$btnNext = New-Object System.Windows.Forms.Button
$btnNext.Text = "&Next >"
$btnNext.Location = New-Object System.Drawing.Point(322, 325)
$btnNext.Size = New-Object System.Drawing.Size(75, 24)
$btnNext.UseVisualStyleBackColor = $true
$form.Controls.Add($btnNext)
$form.AcceptButton = $btnNext

$btnCancel = New-Object System.Windows.Forms.Button
$btnCancel.Text = "Cancel"
$btnCancel.Location = New-Object System.Drawing.Point(410, 325)
$btnCancel.Size = New-Object System.Drawing.Size(75, 24)
$btnCancel.UseVisualStyleBackColor = $true
$btnCancel.DialogResult = [System.Windows.Forms.DialogResult]::Cancel
$form.Controls.Add($btnCancel)

function Refresh-ClassicWizard {{
    $p1.Visible = ($script:page -eq 1)
    $p2.Visible = ($script:page -eq 2)
    $p3.Visible = ($script:page -eq 3)
    $topHeader.Visible = ($script:page -gt 1)
    $topLine.Visible = ($script:page -gt 1)
    $btnBack.Enabled = ($script:page -gt 1)

    if ($script:page -eq 1) {{
        $btnNext.Text = "&Next >"
