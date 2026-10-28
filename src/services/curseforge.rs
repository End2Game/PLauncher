use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha1::Digest;
use std::{fs, path::{Path, PathBuf}, time::Duration};

const API: &str = "https://api.curseforge.com";
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ModSearchResult {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub download_count: u64,
    #[serde(default)]
    pub slug: String,
}
#[derive(Debug, Deserialize)]
struct SearchResponse { data: Vec<ModSearchResult> }
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ModFile {
    pub id: u64,
    #[serde(default)]
    pub display_name: String,
    pub file_name: String,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<FileDependency>,
    #[serde(default)]
    pub is_available: bool,
    #[serde(default)]
    pub hashes: Vec<FileHash>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct FileHash {
    pub value: String,
    pub algo: u32,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct FileDependency {
    pub mod_id: u64,
    pub relation_type: u32,
}
#[derive(Debug, Deserialize)]
struct FilesResponse { data: Vec<ModFile> }

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder().timeout(Duration::from_secs(300))
        .user_agent("PLauncher/1.7.0").build()?)
}
fn api_get<T: for<'de> Deserialize<'de>>(api_key: &str, url: String) -> Result<T> {
    anyhow::ensure!(!api_key.trim().is_empty(), "Вкажи API key CurseForge");
    Ok(client()?.get(url).header("x-api-key", api_key).send()?.error_for_status()?.json()?)
}
pub fn search_mods(api_key: &str, game_version: &str, mod_loader_type: u32, query: &str) -> Result<Vec<ModSearchResult>> {
    let response: SearchResponse = client()?.get(format!("{API}/v1/mods/search"))
        .header("x-api-key", api_key)
        .query(&[
            ("gameId", "432".to_string()), ("classId", "6".to_string()),
            ("gameVersion", game_version.to_string()),
            ("modLoaderType", mod_loader_type.to_string()),
            ("searchFilter", query.to_string()), ("pageSize", "50".to_string())
        ])
        .send()?.error_for_status()?.json().context("Некоректна відповідь CurseForge API")?;
    Ok(response.data)
}
pub fn list_files(api_key: &str, mod_id: u64) -> Result<Vec<ModFile>> {
    let response: FilesResponse = api_get(api_key, format!("{API}/v1/mods/{mod_id}/files?pageSize=50"))?;
    Ok(response.data)
}
/// Selects the newest available file matching both the Minecraft version and loader.
pub fn latest_compatible_file(api_key: &str, mod_id: u64, mc_version: &str, loader_type: u32) -> Result<ModFile> {
    let files = list_files(api_key, mod_id)?;
    files.into_iter().find(|f| f.is_available && f.game_versions.iter().any(|v| v == mc_version)
        && f.game_versions.iter().any(|v| match loader_type {
            1 => v.eq_ignore_ascii_case("forge"),
            4 => v.eq_ignore_ascii_case("fabric"),
            5 => v.eq_ignore_ascii_case("quilt"),
            6 => v.eq_ignore_ascii_case("neoforge"),
            _ => false,
        })).context("CurseForge не має доступного файлу для цієї версії Minecraft і завантажувача")
}
pub fn install_file(api_key: &str, mod_id: u64, file: &ModFile, mods_dir: &Path) -> Result<PathBuf> {
    anyhow::ensure!(!file.file_name.trim().is_empty() && !file.file_name.contains('/') && !file.file_name.contains('\\'),
        "Некоректна назва файлу CurseForge");
    fs::create_dir_all(mods_dir)?;
    let url = match &file.download_url {
        Some(url) if !url.is_empty() => url.clone(),
        _ => {
            #[derive(Deserialize)]
            struct UrlResponse { data: Option<String> }
            let response: UrlResponse = api_get(api_key, format!("{API}/v1/mods/{mod_id}/files/{}/download-url", file.id))?;
            response.data.context("CurseForge не надав URL завантаження для цього файлу")?
        }
    };
    let bytes = client()?.get(url).send()?.error_for_status()?.bytes()?;
    anyhow::ensure!(bytes.starts_with(b"PK"), "Завантажений файл не є JAR/ZIP");
    for hash in &file.hashes {
        let actual = match hash.algo {
            1 => format!("{:x}", sha1::Sha1::digest(&bytes)),
            2 => format!("{:x}", md5::compute(&bytes)),
            _ => continue,
        };
        anyhow::ensure!(actual.eq_ignore_ascii_case(&hash.value),
            "Контрольна сума CurseForge не збігається для {}", file.file_name);
    }
    let destination = mods_dir.join(&file.file_name);
    let tmp = destination.with_extension("jar.part");
    fs::write(&tmp, &bytes)?;
    if destination.exists() { fs::remove_file(&destination)?; }
    fs::rename(tmp, &destination)?;
    Ok(destination)
}
/// Installs a mod and recursively installs required CurseForge dependencies.
/// Optional/incompatible/embedded relations are deliberately not installed automatically.
pub fn install_selected_with_dependencies(
    api_key: &str, mod_id: u64, mc_version: &str, loader_type: u32, mods_dir: &Path
) -> Result<Vec<PathBuf>> {
    let mut visiting = std::collections::HashSet::new();
    let mut installed = std::collections::HashSet::new();
    let mut paths = Vec::new();
    install_recursive(api_key, mod_id, mc_version, loader_type, mods_dir,
        &mut visiting, &mut installed, &mut paths)?;
    Ok(paths)
}

fn install_recursive(
    api_key: &str, mod_id: u64, mc_version: &str, loader_type: u32, mods_dir: &Path,
    visiting: &mut std::collections::HashSet<u64>,
    installed: &mut std::collections::HashSet<u64>,
    paths: &mut Vec<PathBuf>,
) -> Result<()> {
    if installed.contains(&mod_id) { return Ok(()); }
    anyhow::ensure!(visiting.insert(mod_id), "Виявлено цикл залежностей CurseForge для mod ID {mod_id}");
    let file = latest_compatible_file(api_key, mod_id, mc_version, loader_type)
        .with_context(|| format!("Не знайдено сумісний файл для обов'язкової залежності mod ID {mod_id}"))?;

    // Install dependencies first so the target mod is the final item in the dependency order.
    for dependency in file.dependencies.iter().filter(|d| d.relation_type == 3) {
        install_recursive(api_key, dependency.mod_id, mc_version, loader_type, mods_dir,
            visiting, installed, paths)?;
    }

    let path = install_file(api_key, mod_id, &file, mods_dir)?;
    paths.push(path);
    visiting.remove(&mod_id);
    installed.insert(mod_id);
    Ok(())
}

/// Backwards-compatible single-file helper.
pub fn install_selected(api_key: &str, mod_id: u64, mc_version: &str, loader_type: u32, mods_dir: &Path) -> Result<(PathBuf, Vec<FileDependency>)> {
    let file = latest_compatible_file(api_key, mod_id, mc_version, loader_type)?;
    let dependencies = file.dependencies.iter().filter(|d| d.relation_type == 3).cloned().collect();
    let path = install_file(api_key, mod_id, &file, mods_dir)?;
    Ok((path, dependencies))
}
