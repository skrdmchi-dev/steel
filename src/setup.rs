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
