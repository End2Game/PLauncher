use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{path::{Path, PathBuf}, process::{Command, Stdio}};

const PROMOTIONS_URL: &str = "https://neoforged.net/api/gradle/promotions";

#[derive(Debug, Clone, Deserialize)]
pub struct Promotions {
    #[serde(flatten)]
    pub versions: std::collections::HashMap<String, String>,
}

pub fn get_promotions() -> Result<Promotions> {
    let value: serde_json::Value = reqwest::blocking::Client::new()
        .get(PROMOTIONS_URL)
        .header("User-Agent", "PLauncher/1.7.0")
        .send()?.error_for_status()?.json()
        .context("Не вдалося отримати список NeoForge")?;
    let versions = value.as_object().context("Некоректний формат NeoForge API")?
        .iter().filter_map(|(k,v)| v.as_str().map(|s|(k.clone(),s.to_string())))
        .collect();
    Ok(Promotions { versions })
}

/// Builds the official installer artifact URL for a Minecraft/NeoForge version pair.
pub fn installer_url(minecraft_version: &str, neoforge_version: &str) -> String {
    let artifact_version = neoforge_version.to_string();
    format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{artifact_version}/neoforge-{artifact_version}-installer.jar")
}

pub fn download_installer(minecraft_version: &str, neoforge_version: &str, cache_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(cache_dir)?;
    let artifact_version = neoforge_version.to_string();
    let path = cache_dir.join(format!("neoforge-{artifact_version}-installer.jar"));
    let response = reqwest::blocking::Client::new()
        .get(installer_url(minecraft_version, neoforge_version))
        .header("User-Agent", "PLauncher/1.7.0")
        .send()?.error_for_status()
        .context("Не вдалося завантажити NeoForge Installer. Перевір версію Minecraft і NeoForge.")?;
    let bytes = response.bytes()?;
    anyhow::ensure!(bytes.starts_with(b"PK"), "Завантажений файл не схожий на JAR/ZIP");
    let tmp = path.with_extension("jar.tmp");
    std::fs::write(&tmp, &bytes)?;
    if path.exists() { std::fs::remove_file(&path)?; }
    std::fs::rename(tmp, &path)?;
    Ok(path)
}

/// Runs the official NeoForge installer in client mode.
pub fn install_client(java: &str, installer_jar: &Path, game_dir: &Path) -> Result<String> {
    if !installer_jar.is_file() { bail!("NeoForge Installer не знайдено"); }
    std::fs::create_dir_all(game_dir)?;
    let output = Command::new(java)
        .arg("-jar").arg(installer_jar)
        .arg("--installClient").arg(game_dir)
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .output().context("Не вдалося запустити Java/NeoForge Installer")?;
    if !output.status.success() {
        bail!("NeoForge Installer завершився з помилкою:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }
    let versions = game_dir.join("versions");
    let mut profiles = std::fs::read_dir(&versions).context("NeoForge не створив каталог versions")?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| p.is_dir() && p.file_name().and_then(|s| s.to_str()).map(|s| s.to_lowercase().contains("neoforge")).unwrap_or(false))
        .collect::<Vec<_>>();
    profiles.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    let profile = profiles.into_iter().rev().find(|p| {
        let id = p.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        p.join(format!("{id}.json")).is_file()
    }).context("Installer завершився, але NeoForge profile JSON не знайдено")?;
    Ok(profile.file_name().unwrap().to_string_lossy().to_string())
}


/// Checks whether the exact Minecraft/NeoForge pair has an official installer artifact.
/// This is an artifact-availability check, not a guarantee that every mod is compatible.
pub fn check_compatibility(minecraft_version: &str, neoforge_version: &str) -> Result<String> {
    anyhow::ensure!(!minecraft_version.trim().is_empty() && !neoforge_version.trim().is_empty(),
        "Вкажи обидві версії");
    anyhow::ensure!(!minecraft_version.contains('/') && !neoforge_version.contains('/'),
        "Некоректний номер версії");
    let artifact_version = neoforge_version.to_string();
    let expected_prefix = if minecraft_version == "1.20.1" {
        vec!["47.1.", "20.1."]
    } else {
        vec![minecraft_version.strip_prefix("1.").unwrap_or(minecraft_version)]
    };
    anyhow::ensure!(expected_prefix.iter().any(|prefix| neoforge_version.starts_with(prefix)),
        "Версія NeoForge {neoforge_version} не відповідає Minecraft {minecraft_version}");
    let url = installer_url(minecraft_version, neoforge_version);
    let response = reqwest::blocking::Client::new()
        .head(&url).header("User-Agent", "PLauncher/1.7.0").send()
        .context("Не вдалося перевірити Maven NeoForge")?;
    if !response.status().is_success() {
        bail!("NeoForge {artifact_version} не знайдено для Minecraft {minecraft_version} (HTTP {})",
            response.status());
    }
    Ok(format!("Офіційний installer існує: Minecraft {minecraft_version} · NeoForge {artifact_version}"))
}
