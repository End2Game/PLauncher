use anyhow::{bail, Context, Result};
use std::{fs::File, path::{Path, PathBuf}};
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, ZipWriter};

pub fn backup_directory(source: &str) -> Result<PathBuf> {
    let root = Path::new(source);
    if !root.is_dir() { bail!("Вкажи наявну папку інстансу"); }
    let name = root.file_name().context("Некоректний шлях")?.to_string_lossy();
    let output = root.with_file_name(format!("{name}-backup-{}.zip", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    let file = File::create(&output).context("Не вдалося створити ZIP")?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(root)?;
            zip.start_file(rel.to_string_lossy().replace('\\', "/"), options)?;
            let mut input = File::open(entry.path())?;
            std::io::copy(&mut input, &mut zip)?;
        }
    }
    zip.finish()?;
    Ok(output)
}
