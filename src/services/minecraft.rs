use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::{fs, io::Write, path::{Component, Path, PathBuf}, time::Duration};

const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
#[derive(Debug, Clone, Deserialize)]
pub struct VersionInfo { pub id: String, #[serde(rename="type")] pub kind: String, #[serde(rename="releaseTime")] pub release_time: String, pub url: String }
#[derive(Deserialize)] struct Manifest { versions: Vec<VersionInfo> }
#[derive(Deserialize)] struct VersionMeta { id: String, downloads: Option<Downloads>, libraries: Option<Vec<Library>>, #[serde(rename="assetIndex")] asset_index: Option<AssetIndex> }
#[derive(Deserialize)] struct Downloads { client: Option<Download> }
#[derive(Deserialize)] struct Download { url: String, sha1: Option<String>, path: Option<String> }
#[derive(Deserialize)] struct Library { downloads: Option<LibraryDownloads> }
#[derive(Deserialize)] struct LibraryDownloads { artifact: Option<Download> }
#[derive(Deserialize)] struct AssetIndex { id: String, url: String, sha1: Option<String> }
#[derive(Deserialize)] struct Assets { objects: std::collections::HashMap<String, AssetObject> }
#[derive(Deserialize)] struct AssetObject { hash: String, size: Option<u64> }

pub fn get_versions() -> Result<Vec<VersionInfo>> {
    let client = http()?; let manifest: Manifest = client.get(MANIFEST).send()?.error_for_status()?.json()?; Ok(manifest.versions)
}
pub fn install_vanilla(version: &VersionInfo, game_dir: &Path, progress: &mut dyn FnMut(String)) -> Result<()> {
    let client = http()?; let mc = game_dir.join(".minecraft");
    progress(format!("Завантаження метаданих {}", version.id));
    let meta_bytes = client.get(&version.url).send()?.error_for_status()?.bytes()?;
    let meta_path = mc.join("versions").join(&version.id).join(format!("{}.json", version.id));
    write_atomic(&meta_path, &meta_bytes)?;
    let meta: VersionMeta = serde_json::from_slice(&meta_bytes)?;
    let client_download = meta.downloads.and_then(|d| d.client).context("У маніфесті немає client download")?;
    let client_path = mc.join("versions").join(&meta.id).join(format!("{}.jar", meta.id));
    fetch_verified(&client, &client_download.url, &client_path, client_download.sha1.as_deref())?;

    let libraries = meta.libraries.unwrap_or_default();
    let total = libraries.len();
    for (i, lib) in libraries.iter().enumerate() {
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            if let Some(rel) = artifact.path.as_deref() {
                let rel_path = safe_relative(rel)?;
                progress(format!("Бібліотеки: {}/{}", i + 1, total));
                fetch_verified(&client, &artifact.url, &mc.join("libraries").join(rel_path), artifact.sha1.as_deref())?;
            }
        }
    }
    if let Some(index) = meta.asset_index {
        progress("Завантаження індексу ресурсів".into());
        let bytes = client.get(&index.url).send()?.error_for_status()?.bytes()?;
        verify_sha1(&bytes, index.sha1.as_deref())?;
        let index_path = mc.join("assets/indexes").join(format!("{}.json", index.id)); write_atomic(&index_path, &bytes)?;
        let assets: Assets = serde_json::from_slice(&bytes)?;
        let total = assets.objects.len();
        for (i, obj) in assets.objects.values().enumerate() {
            if obj.hash.len() != 40 || !obj.hash.bytes().all(|b| b.is_ascii_hexdigit()) { bail!("Некоректний SHA-1 у маніфесті ресурсів"); }
            let prefix = &obj.hash[..2];
            let url = format!("https://resources.download.minecraft.net/{}/{}", prefix, obj.hash);
            let path = mc.join("assets/objects").join(prefix).join(&obj.hash);
            if !path.is_file() || !file_hash_matches(&path, &obj.hash)? {
                fetch_verified(&client, &url, &path, Some(&obj.hash))?;
            }
            if i % 100 == 0 { progress(format!("Ресурси: {}/{}", i + 1, total)); }
            let _ = obj.size;
        }
    }
    progress("Базові файли Minecraft встановлено".into()); Ok(())
}
fn http() -> Result<reqwest::blocking::Client> { Ok(reqwest::blocking::Client::builder().timeout(Duration::from_secs(600)).user_agent("PLauncher/0.3.0").build()?) }
fn safe_relative(s: &str) -> Result<PathBuf> { let p = Path::new(s); if p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) { bail!("Небезпечний шлях у метаданих: {s}"); } Ok(p.to_path_buf()) }
fn fetch_verified(client: &reqwest::blocking::Client, url: &str, path: &Path, expected: Option<&str>) -> Result<()> {
    if path.is_file() && expected.map(|h| file_hash_matches(path,h).unwrap_or(false)).unwrap_or(true) { return Ok(()); }
    let bytes = client.get(url).send()?.error_for_status()?.bytes()?; verify_sha1(&bytes, expected)?; write_atomic(path, &bytes)
}
fn verify_sha1(bytes: &[u8], expected: Option<&str>) -> Result<()> { if let Some(hash) = expected { let actual = format!("{:x}", Sha1::digest(bytes)); if !actual.eq_ignore_ascii_case(hash) { bail!("SHA-1 не збігається: очікувався {hash}, отримано {actual}"); } } Ok(()) }
fn file_hash_matches(path: &Path, expected: &str) -> Result<bool> { let bytes = fs::read(path)?; Ok(format!("{:x}", Sha1::digest(bytes)).eq_ignore_ascii_case(expected)) }
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> { let parent = path.parent().context("Некоректний шлях файлу")?; fs::create_dir_all(parent)?; let tmp = path.with_extension(format!("{}.part", path.extension().and_then(|x|x.to_str()).unwrap_or("tmp"))); let mut f = fs::File::create(&tmp)?; f.write_all(bytes)?; f.sync_all()?; fs::rename(&tmp, path)?; Ok(()) }
