use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub assets: Vec<GithubAsset>,
    pub prerelease: bool,
    pub draft: bool,
}
#[derive(Debug, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

pub fn latest_release(owner: &str, repo: &str) -> Result<GithubRelease> {
    anyhow::ensure!(!owner.trim().is_empty() && !repo.trim().is_empty(),
        "Спочатку вкажи власника та назву GitHub-репозиторію");
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let release: GithubRelease = reqwest::blocking::Client::new()
        .get(url)
        .header("User-Agent", "PLauncher-Updater")
        .header("Accept", "application/vnd.github+json")
        .send()?.error_for_status()?.json().context("Не вдалося прочитати GitHub Release")?;
    if release.draft || release.prerelease { bail!("GitHub повернув не стабільний реліз"); }
    Ok(release)
}
