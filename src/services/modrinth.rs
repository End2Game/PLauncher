use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha512};
use std::{fs, path::{Path, PathBuf}};

const API: &str = "https://api.modrinth.com/v2";
const USER_AGENT: &str = "PLauncher/0.5.0";

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse { pub hits: Vec<ProjectHit>, pub total_hits: u32 }
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectHit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub project_type: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<VersionFile>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub hashes: Hashes,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Hashes { pub sha512: Option<String>, pub sha1: Option<String> }

pub fn search_mods(query: &str, game_version: &str, loader: &str, offset: u32) -> Result<SearchResponse> {
    let client = reqwest::blocking::Client::builder().user_agent(USER_AGENT).build()?;
    let facets = serde_json::json!([["project_type:mod"], [format!("versions:{game_version}")], [format!("categories:{loader}") ]]);
    let url = format!("{API}/search");
    let response = client.get(url).query(&[
        ("query", query), ("limit", "20"), ("offset", &offset.to_string()),
        ("facets", &facets.to_string()),
    ]).send()?.error_for_status()?;
    Ok(response.json()?)
}

pub fn get_project_versions(project_id: &str, game_version: &str, loader: &str) -> Result<Vec<ProjectVersion>> {
    let client = reqwest::blocking::Client::builder().user_agent(USER_AGENT).build()?;
    let url = format!("{API}/project/{project_id}/version");
    let response = client.get(url).query(&[
        ("game_versions", serde_json::json!([game_version]).to_string()),
        ("loaders", serde_json::json!([loader]).to_string()),
    ]).send()?.error_for_status()?;
    Ok(response.json()?)
}

pub fn install_latest(project_id: &str, game_version: &str, loader: &str, mods_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(mods_dir)?;
    let versions = get_project_versions(project_id, game_version, loader)?;
    let version = versions.first().ok_or_else(|| anyhow!("Сумісних версій мода не знайдено"))?;
    let file = version.files.iter().find(|f| f.primary).or_else(|| version.files.first())
        .ok_or_else(|| anyhow!("У версії немає файлу для завантаження"))?;
    let safe_name = Path::new(&file.filename).file_name().ok_or_else(|| anyhow!("Некоректна назва файлу"))?;
    let destination = mods_dir.join(safe_name);
    if destination.exists() { return Err(anyhow!("Файл уже існує: {}", destination.display())); }
    let client = reqwest::blocking::Client::builder().user_agent(USER_AGENT).build()?;
    let bytes = client.get(&file.url).send()?.error_for_status()?.bytes()?;
    if let Some(expected) = &file.hashes.sha512 {
        let actual = format!("{:x}", Sha512::digest(&bytes));
        if !actual.eq_ignore_ascii_case(expected) { return Err(anyhow!("SHA-512 не збігається; файл відхилено")); }
    } else if let Some(expected) = &file.hashes.sha1 {
        use sha1::{Digest as Sha1Digest, Sha1};
        let actual = format!("{:x}", Sha1::digest(&bytes));
        if !actual.eq_ignore_ascii_case(expected) { return Err(anyhow!("SHA-1 не збігається; файл відхилено")); }
    }
    let temp = destination.with_extension("download.tmp");
    fs::write(&temp, &bytes).context("Не вдалося записати тимчасовий файл")?;
    fs::rename(&temp, &destination).context("Не вдалося атомарно зберегти мод")?;
    Ok(destination)
}

pub fn list_installed(mods_dir: &Path) -> Result<Vec<PathBuf>> {
    if !mods_dir.exists() { return Ok(Vec::new()); }
    let mut files = fs::read_dir(mods_dir)?.filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| p.is_file() && p.extension().map(|x| x.to_string_lossy().eq_ignore_ascii_case("jar")).unwrap_or(false))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

pub fn remove_mod(mods_dir: &Path, filename: &str) -> Result<()> {
    let name = Path::new(filename).file_name().ok_or_else(|| anyhow!("Некоректна назва"))?;
    let target = mods_dir.join(name);
    if target.parent() != Some(mods_dir) { return Err(anyhow!("Недозволений шлях")); }
    fs::remove_file(target)?;
    Ok(())
}
