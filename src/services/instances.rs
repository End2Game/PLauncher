use crate::models::GameInstance;
use anyhow::{bail, Context, Result};
use std::{fs, path::{Component, Path, PathBuf}};

pub struct InstanceService { root: PathBuf }
impl InstanceService {
    pub fn new(root: PathBuf) -> Result<Self> { fs::create_dir_all(&root)?; Ok(Self { root }) }
    pub fn root(&self) -> &Path { &self.root }
    pub fn load_all(&self) -> Result<Vec<GameInstance>> {
        let mut items = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path().join("instance.json");
            if !path.is_file() { continue; }
            if let Ok(bytes) = fs::read(&path) { if let Ok(item) = serde_json::from_slice::<GameInstance>(&bytes) { items.push(item); } }
        }
        items.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase())); Ok(items)
    }
    pub fn create(&self, name: &str, version: &str) -> Result<GameInstance> {
        let clean = name.trim();
        if clean.is_empty() || clean.len() > 48 || clean.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)) || clean == "." || clean == ".." { bail!("Вкажи коректну назву інстансу"); }
        let path = self.root.join(clean);
        if path.exists() { bail!("Інстанс із такою назвою вже існує"); }
        for part in Path::new(clean).components() { if !matches!(part, Component::Normal(_)) { bail!("Некоректна назва"); } }
        fs::create_dir_all(path.join(".minecraft"))?;
        for dir in ["mods", "config", "shaderpacks", "resourcepacks", "saves"] { fs::create_dir_all(path.join(".minecraft").join(dir))?; }
        let item = GameInstance { name: clean.into(), minecraft_version: version.into(), loader: "Vanilla".into(), directory: path, installed: false };
        self.save(&item)?; Ok(item)
    }
    pub fn save(&self, item: &GameInstance) -> Result<()> {
        if !item.directory.starts_with(&self.root) { bail!("Шлях інстансу виходить за межі каталогу лаунчера"); }
        fs::create_dir_all(&item.directory)?;
        let target = item.directory.join("instance.json");
        let tmp = item.directory.join("instance.json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(item)?)?;
        fs::rename(&tmp, &target).with_context(|| format!("Не вдалося зберегти {}", target.display()))?; Ok(())
    }
}
