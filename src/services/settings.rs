use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub oauth_client_id: String,
    pub theme: String,
    pub download_concurrency: usize,
    pub default_min_ram_mb: u32,
    pub default_max_ram_mb: u32,
    pub github_owner: String,
    pub github_repo: String,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self { oauth_client_id: String::new(), theme: "dark".into(),
            download_concurrency: 4, default_min_ram_mb: 1024,
            default_max_ram_mb: 4096, github_owner: String::new(), github_repo: String::new() }
    }
}
pub fn settings_path() -> PathBuf {
    directories::ProjectDirs::from("ua", "PLauncher", "PLauncher")
        .map(|p| p.config_dir().join("settings.json"))
        .unwrap_or_else(|| PathBuf::from("PLauncherData/settings.json"))
}
pub fn load() -> AppSettings {
    fs::read(settings_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
pub fn save(settings: &AppSettings) -> Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    if path.exists() { fs::remove_file(&path)?; }
    fs::rename(tmp, path)?;
    Ok(())
}
