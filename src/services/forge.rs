use anyhow::{bail, Context, Result};
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}, time::Duration};

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder().timeout(Duration::from_secs(600))
        .user_agent("PLauncher/1.7.0").build()?)
}

/// Downloads and runs the official Forge installer for the selected MC/Forge pair.
/// Forge installer creates a version JSON with its own libraries and launch arguments.
pub fn install_forge(java: &str, mc_version: &str, forge_version: &str, game_dir: &Path) -> Result<String> {
    anyhow::ensure!(!mc_version.trim().is_empty() && !forge_version.trim().is_empty(), "Вкажи версії Minecraft і Forge");
    anyhow::ensure!(!mc_version.contains('/') && !mc_version.contains('\\') && !forge_version.contains('/') && !forge_version.contains('\\'), "Некоректний номер версії");
    fs::create_dir_all(game_dir)?;
    let artifact = format!("{mc_version}-{forge_version}");
    let url = format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{artifact}/forge-{artifact}-installer.jar");
    let cache = game_dir.join("launcher_cache");
    fs::create_dir_all(&cache)?;
    let jar = cache.join(format!("forge-{artifact}-installer.jar"));
    let response = client()?.get(url).send()?.error_for_status().context("Forge installer не знайдено. Перевір пару версій Minecraft/Forge.")?;
    let bytes = response.bytes()?;
    anyhow::ensure!(bytes.starts_with(b"PK"), "Сервер Forge повернув не JAR-файл");
    let tmp = jar.with_extension("jar.part");
    fs::write(&tmp, &bytes)?;
    if jar.exists() { fs::remove_file(&jar)?; }
    fs::rename(tmp, &jar)?;
    let output = Command::new(java).arg("-jar").arg(&jar).arg("--installClient").arg(game_dir)
        .stdout(Stdio::piped()).stderr(Stdio::piped()).output()
        .context("Не вдалося запустити Forge installer через Java")?;
    if !output.status.success() {
        bail!("Forge installer завершився з кодом {:?}:\n{}\n{}",
            output.status.code(), String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }
    let versions = game_dir.join("versions");
    let mut profiles: Vec<PathBuf> = fs::read_dir(&versions).with_context(|| "Forge не створив каталог versions")?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| p.is_dir() && p.file_name().and_then(|s| s.to_str()).map(|s| s.starts_with(&format!("{mc_version}-forge-"))).unwrap_or(false))
        .collect();
    profiles.sort();
    let profile = profiles.into_iter().rev().find(|p| p.join(format!("{}.json", p.file_name().unwrap().to_string_lossy())).is_file())
        .context("Installer завершився, але Forge profile JSON не знайдено")?;
    Ok(profile.file_name().unwrap().to_string_lossy().to_string())
}
