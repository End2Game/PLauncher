use anyhow::{bail, Context, Result};
use std::{fs::File, path::{Component, Path, PathBuf}};
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

pub fn backup_world(world_dir: &Path, backup_dir: &Path) -> Result<PathBuf> {
    if !world_dir.is_dir() { bail!("Папку світу не знайдено"); }
    std::fs::create_dir_all(backup_dir)?;
    let name = world_dir.file_name().context("Некоректна назва світу")?.to_string_lossy();
    let output = backup_dir.join(format!("{}-{}.zip", name, chrono::Local::now().format("%Y%m%d-%H%M%S")));
    let file = File::create(&output)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in WalkDir::new(world_dir).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(world_dir)?;
            zip.start_file(rel.to_string_lossy().replace('\\', "/"), options)?;
            let mut input = File::open(entry.path())?;
            std::io::copy(&mut input, &mut zip)?;
        }
    }
    zip.finish()?;
    Ok(output)
}

pub fn restore_world(archive_path: &Path, destination: &Path) -> Result<usize> {
    let file = File::open(archive_path).context("Не вдалося відкрити архів")?;
    let mut archive = ZipArchive::new(file).context("Некоректний ZIP-архів")?;
    std::fs::create_dir_all(destination)?;
    let root = destination.canonicalize()?;
    let mut restored = 0usize;
    for i in 0..archive.len() {
        let mut item = archive.by_index(i)?;
        let Some(enclosed) = item.enclosed_name().map(Path::to_path_buf) else {
            bail!("Архів містить небезпечний шлях");
        };
        if enclosed.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
            bail!("Архів містить вихід за межі каталогу");
        }
        let out = destination.join(enclosed);
        if item.is_dir() { std::fs::create_dir_all(&out)?; continue; }
        if let Some(parent) = out.parent() { std::fs::create_dir_all(parent)?; }
        let parent = out.parent().context("Некоректний шлях файлу")?.canonicalize()?;
        if !parent.starts_with(&root) { bail!("Небезпечний шлях відновлення"); }
        let mut target = File::create(&out)?;
        std::io::copy(&mut item, &mut target)?;
        restored += 1;
    }
    Ok(restored)
}
