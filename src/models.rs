use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInstance {
    pub name: String,
    pub minecraft_version: String,
    pub loader: String,
    pub directory: PathBuf,
    #[serde(default)]
    pub installed: bool,
}
