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
