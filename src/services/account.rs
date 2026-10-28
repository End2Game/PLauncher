use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}, thread, time::Duration};

const DEVICE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftAccount {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Deserialize)]
struct DeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    message: String,
    interval: Option<u64>,
    expires_in: u64,
}
#[derive(Deserialize)]
struct OAuthToken {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    error: Option<String>,
}
#[derive(Deserialize)]
struct XboxToken { Token: String }
#[derive(Deserialize)]
struct MinecraftLogin { access_token: String }
#[derive(Deserialize)]
struct MinecraftProfile { id: String, name: String }

#[derive(Debug, Clone)]
pub struct DeviceLogin {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub message: String,
    pub interval: u64,
    pub expires_in: u64,
}

pub fn begin_device_login(client_id: &str) -> Result<DeviceLogin> {
    if client_id.trim().is_empty() { bail!("Вкажи Microsoft OAuth Client ID у налаштуваннях"); }
    let c = reqwest::blocking::Client::new();
    let d: DeviceCode = c.post(DEVICE_URL)
        .form(&[("client_id", client_id), ("scope", "XboxLive.signin offline_access")])
        .send()?.error_for_status()?.json()?;
    Ok(DeviceLogin {
        device_code: d.device_code,
        user_code: d.user_code,
        verification_uri: d.verification_uri,
        message: d.message,
        interval: d.interval.unwrap_or(5).max(1),
        expires_in: d.expires_in,
    })
}

pub fn complete_device_login(client_id: &str, device: &DeviceLogin) -> Result<MinecraftAccount> {
    if client_id.trim().is_empty() { bail!("Не задано Microsoft OAuth Client ID"); }
    let c = reqwest::blocking::Client::builder().timeout(Duration::from_secs(30)).build()?;
    let mut interval = device.interval.max(1);
    let mut oauth: Option<OAuthToken> = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(device.expires_in);
    while std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_secs(interval));
        let response = c.post(TOKEN_URL).form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", client_id),
            ("device_code", &device.device_code),
        ]).send()?;
        let value: serde_json::Value = response.json()?;
        if let Some(token) = value.get("access_token").and_then(|v| v.as_str()) {
            oauth = Some(OAuthToken {
                access_token: token.to_string(),
                refresh_token: value.get("refresh_token").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                error: None,
            });
            break;
        }
        match value.get("error").and_then(|v| v.as_str()).unwrap_or("") {
            "authorization_pending" => continue,
            "slow_down" => { interval += 5; continue; },
            "authorization_declined" => bail!("Вхід Microsoft відхилено"),
            "expired_token" => bail!("Код входу Microsoft прострочено"),
            e => bail!("Microsoft OAuth: {e}"),
        }
    }
    let oauth = oauth.context("Час очікування Microsoft OAuth вичерпано")?;

    let xbl: XboxToken = c.post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&serde_json::json!({
            "Properties": {"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={}", oauth.access_token)},
            "RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"
        })).send()?.error_for_status()?.json()?;
    let xsts: serde_json::Value = c.post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&serde_json::json!({
            "Properties":{"SandboxId":"RETAIL","UserTokens":[xbl.Token]},
            "RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"
        })).send()?.error_for_status()?.json()?;
    let xsts_token = xsts.get("Token").and_then(|v| v.as_str()).context("XSTS не повернув токен")?;
    let user_hash = xsts.pointer("/DisplayClaims/xui/0/uhs").and_then(|v| v.as_str()).context("XSTS не повернув user hash")?;
    let mc: MinecraftLogin = c.post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&serde_json::json!({"identityToken":format!("XBL3.0 x={user_hash};{xsts_token}")}))
        .send()?.error_for_status()?.json()?;
    let profile: MinecraftProfile = c.get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc.access_token).send()?.error_for_status()?.json()
        .context("Не знайдено профіль Minecraft. Перевір, чи є придбана гра на акаунті.")?;
    Ok(MinecraftAccount { username: profile.name, uuid: profile.id, access_token: mc.access_token, refresh_token: oauth.refresh_token })
}

// Compatibility helper for callers that still want the entire flow in one call.
pub fn login_device(client_id: &str) -> Result<MinecraftAccount> {
    let device = begin_device_login(client_id)?;
    complete_device_login(client_id, &device)
}

pub fn save_account(path: &Path, account: &MinecraftAccount) -> Result<()> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(account)?)?;
    if path.exists() { fs::remove_file(path)?; }
    fs::rename(tmp, path)?;
    Ok(())
}
pub fn load_account(path: &Path) -> Result<MinecraftAccount> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
pub fn account_path() -> PathBuf {
    directories::ProjectDirs::from("ua", "PLauncher", "PLauncher")
        .map(|p| p.data_dir().join("account.json"))
        .unwrap_or_else(|| PathBuf::from("PLauncherData/account.json"))
}
