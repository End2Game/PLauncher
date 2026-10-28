use anyhow::Result;
use std::{collections::HashMap, fs, path::Path};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ModConflict {
    pub file_a: String,
    pub file_b: String,
    pub reason: String,
}
pub fn scan_mod_directory(mods_dir: &Path) -> Result<Vec<ModConflict>> {
    let mut by_hash: HashMap<String, Vec<String>> = HashMap::new();
    let mut by_name: HashMap<String, String> = HashMap::new();
    for entry in WalkDir::new(mods_dir).max_depth(1).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.path().extension().and_then(|x| x.to_str()) != Some("jar") { continue; }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let bytes = fs::read(entry.path())?;
        let hash = format!("{:x}", md5::compute(&bytes));
        by_hash.entry(hash).or_default().push(name.clone());
        by_name.insert(name, entry.path().display().to_string());
    }
    let mut conflicts = Vec::new();
    for files in by_hash.values().filter(|v| v.len() > 1) {
        for pair in files.windows(2) {
            conflicts.push(ModConflict { file_a: pair[0].clone(), file_b: pair[1].clone(), reason: "Однаковий вміст JAR (дубль)".into() });
        }
    }
    // Heuristic warnings only: filename matches are not proof of incompatibility.
    let names: Vec<_> = by_name.keys().cloned().collect();
    for (i, a) in names.iter().enumerate() {
        for b in names.iter().skip(i + 1) {
            let stem_a = a.trim_end_matches(".jar");
            let stem_b = b.trim_end_matches(".jar");
            if stem_a == stem_b && a != b {
                conflicts.push(ModConflict { file_a: a.clone(), file_b: b.clone(), reason: "Схожі назви — перевір вручну".into() });
            }
        }
    }
    Ok(conflicts)
}
