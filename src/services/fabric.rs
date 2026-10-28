use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{fs, path::{Component, Path, PathBuf}, time::Duration};

#[derive(Debug, Clone, Deserialize)]
pub struct FabricLoader {
    pub loader: FabricLoaderInfo,
    pub intermediary: Option<FabricIntermediary>,
    pub launcher: Option<serde_json::Value>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct FabricLoaderInfo {
    pub version: String,
    pub stable: Option<bool>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct FabricIntermediary { pub version: String }

#[derive(Debug, Clone, Deserialize)]
struct FabricProfile {
    id: String,
    #[serde(default, rename="inheritsFrom")]
    inherits_from: String,
    #[serde(default)]
    libraries: Vec<FabricLibrary>,
}
#[derive(Debug, Clone, Deserialize)]
struct FabricLibrary {
    name: String,
    url: Option<String>,
    downloads: Option<FabricDownloads>,
}
#[derive(Debug, Clone, Deserialize)]
struct FabricDownloads { artifact: Option<FabricArtifact> }
#[derive(Debug, Clone, Deserialize)]
struct FabricArtifact { path: Option<String>, url: String, sha1: Option<String> }

pub fn list_loaders(mc_version: &str) -> Result<Vec<FabricLoader>> {
    let client = client()?;
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc_version}");
    Ok(client.get(url).send()?.error_for_status()?.json()?)
}

pub fn install_fabric(mc_version: &str, instance_dir: &Path) -> Result<String> {
    let client = client()?;
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc_version}/profile/json");
    let response = client.get(url).send()?.error_for_status()?;
    let bytes = response.bytes()?;
    let profile: FabricProfile = serde_json::from_slice(&bytes).context("Некоректний профіль Fabric")?;
    if profile.inherits_from != mc_version {
        bail!("Fabric profile посилається на іншу версію Minecraft");
    }
    if profile.id.is_empty() || profile.id.contains('/') || profile.id.contains('\\') {
        bail!("Некоректний ID Fabric profile");
    }

    let mc = instance_dir.join(".minecraft");
    let version_dir = mc.join("versions").join(&profile.id);
    fs::create_dir_all(&version_dir)?;
    let version_json = version_dir.join(format!("{}.json", profile.id));
    let tmp = version_json.with_extension("json.part");
    fs::write(&tmp, &bytes)?;
    fs::rename(tmp, version_json)?;

    for library in &profile.libraries {
        let artifact = library.downloads.as_ref().and_then(|d| d.artifact.as_ref());
        let Some(artifact) = artifact else { continue; };
        let rel = artifact.path.as_deref().unwrap_or_else(|| "");
        if rel.is_empty() { continue; }
        let rel_path = safe_path(rel)?;
        let destination = mc.join("libraries").join(rel_path);
        if destination.is_file() && artifact.sha1.as_deref().map(|h| hash_file(&destination, h).unwrap_or(false)).unwrap_or(true) {
            continue;
        }
        let response = client.get(&artifact.url).send()?.error_for_status()?;
        let data = response.bytes()?;
        if let Some(expected) = &artifact.sha1 {
            let actual = format!("{:x}", sha1::Sha1::digest(&data));
            if !actual.eq_ignore_ascii_case(expected) {
                bail!("SHA-1 бібліотеки Fabric не збігається: {}", library.name);
            }
        }
        if let Some(parent) = destination.parent() { fs::create_dir_all(parent)?; }
        let temp = destination.with_extension("fabric-part");
        fs::write(&temp, &data)?;
        if destination.exists() { fs::remove_file(&destination)?; }
        fs::rename(temp, destination)?;
    }
    Ok(profile.id)
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent("PLauncher/0.7.0")
        .build()?)
}
fn safe_path(value: &str) -> Result<PathBuf> {
    let p = Path::new(value);
    if p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        bail!("Небезпечний шлях у метаданих Fabric: {value}");
    }
    Ok(p.to_path_buf())
}
fn hash_file(path: &Path, expected: &str) -> Result<bool> {
    use sha1::Digest;
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", sha1::Sha1::digest(bytes)).eq_ignore_ascii_case(expected))
}
