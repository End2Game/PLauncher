use anyhow::{anyhow, Context, Result};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha512;
use std::{fs, io::{Cursor, Read}, path::{Component, Path, PathBuf}};
use zip::ZipArchive;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MrpackIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: Option<String>,
    pub name: Option<String>,
    pub files: Vec<MrpackFile>,
    pub dependencies: std::collections::HashMap<String, String>,
}
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MrpackFile {
    pub path: String,
    pub hashes: MrpackHashes,
    pub downloads: Vec<String>,
    pub file_size: u64,
    pub env: Option<MrpackEnv>,
}
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MrpackHashes {
    pub sha1: Option<String>,
    pub sha512: Option<String>,
}
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MrpackEnv {
    pub client: Option<String>,
    pub server: Option<String>,
}
#[derive(Debug, Clone)]
pub struct MrpackReport {
    pub name: String,
    pub minecraft_version: String,
    pub loader: String,
    pub installed_files: usize,
    pub skipped_files: usize,
}

fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(anyhow!("Недозволений шлях у .mrpack: {value}"));
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(anyhow!("Небезпечний шлях у .mrpack: {value}"));
        }
    }
    Ok(path.to_path_buf())
}

pub fn inspect_mrpack(path: &Path) -> Result<MrpackIndex> {
    let bytes = fs::read(path).with_context(|| format!("Не вдалося прочитати {}", path.display()))?;
    let mut archive = ZipArchive::new(Cursor::new(bytes)).context("Файл не є коректним ZIP/.mrpack")?;
    let mut file = archive.by_name("modrinth.index.json").context("В архіві немає modrinth.index.json")?;
    let mut json = String::new();
    file.read_to_string(&mut json)?;
    let index: MrpackIndex = serde_json::from_str(&json).context("Некоректний modrinth.index.json")?;
    if index.format_version != 1 { return Err(anyhow!("Непідтримувана версія формату mrpack: {}", index.format_version)); }
    if index.game != "minecraft" { return Err(anyhow!("Цей пакет не призначений для Minecraft")); }
    Ok(index)
}

pub fn import_mrpack(path: &Path, instance_dir: &Path) -> Result<MrpackReport> {
    let archive_bytes = fs::read(path)?;
    let mut archive = ZipArchive::new(Cursor::new(archive_bytes))?;
    let mut index_json = String::new();
    archive.by_name("modrinth.index.json")?.read_to_string(&mut index_json)?;
    let index: MrpackIndex = serde_json::from_str(&index_json)?;
    if index.format_version != 1 || index.game != "minecraft" {
        return Err(anyhow!("Непідтримуваний формат або гра в .mrpack"));
    }

    let deps = &index.dependencies;
    let mc = deps.get("minecraft").cloned().unwrap_or_default();
    if mc.is_empty() { return Err(anyhow!("У manifest відсутня версія Minecraft")); }
    let loader = ["fabric", "forge", "neoforge", "quilt"]
        .iter().find(|name| deps.contains_key(**name))
        .map(|s| (*s).to_string()).unwrap_or_else(|| "vanilla".to_string());

    let mut installed = 0usize;
    let mut skipped = 0usize;
    let client = reqwest::blocking::Client::builder()
        .user_agent("PLauncher/0.5.0 (mrpack importer)")
        .build()?;

    for entry in &index.files {
        let relative = safe_relative_path(&entry.path)?;
        if entry.env.as_ref().and_then(|e| e.client.as_deref()) == Some("unsupported") {
            skipped += 1;
            continue;
        }
        let Some(url) = entry.downloads.first() else { skipped += 1; continue; };
        let response = client.get(url).send()?.error_for_status()?;
        let bytes = response.bytes()?;
        if entry.file_size > 0 && bytes.len() as u64 != entry.file_size {
            return Err(anyhow!("Розмір файлу не збігається: {}", entry.path));
        }
        if let Some(expected) = &entry.hashes.sha512 {
            let actual = format!("{:x}", Sha512::digest(&bytes));
            if !actual.eq_ignore_ascii_case(expected) { return Err(anyhow!("SHA-512 не збігається: {}", entry.path)); }
        } else if let Some(expected) = &entry.hashes.sha1 {
            let actual = format!("{:x}", Sha1::digest(&bytes));
            if !actual.eq_ignore_ascii_case(expected) { return Err(anyhow!("SHA-1 не збігається: {}", entry.path)); }
        }

        let destination = instance_dir.join(&relative);
        if !destination.starts_with(instance_dir) { return Err(anyhow!("Шлях виходить за межі інстансу")); }
        if let Some(parent) = destination.parent() { fs::create_dir_all(parent)?; }
        let temp = destination.with_extension("plauncher-tmp");
        fs::write(&temp, &bytes)?;
        if destination.exists() { fs::remove_file(&destination)?; }
        fs::rename(&temp, &destination)?;
        installed += 1;
    }

    // Optional overrides are copied only after validating every archive path.
    let mut overrides = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if let Some(rest) = name.strip_prefix("overrides/") {
            if rest.is_empty() { continue; }
            let relative = safe_relative_path(rest)?;
            if file.is_dir() { continue; }
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            overrides.push((relative, bytes));
        }
    }
    for (relative, bytes) in overrides {
        let destination = instance_dir.join(relative);
        if !destination.starts_with(instance_dir) { return Err(anyhow!("Небезпечний шлях overrides")); }
        if let Some(parent) = destination.parent() { fs::create_dir_all(parent)?; }
        fs::write(destination, bytes)?;
    }

    Ok(MrpackReport {
        name: index.name.unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().to_string()),
        minecraft_version: mc,
        loader,
        installed_files: installed,
        skipped_files: skipped,
    })
}
