use anyhow::{Context, Result};
use std::{path::PathBuf, process::Command};

#[derive(Debug, Clone)]
pub struct JavaInfo { pub executable: PathBuf, pub version: String }

pub fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = std::env::var_os("JAVA_HOME") { out.push(PathBuf::from(home).join("bin").join(if cfg!(windows) {"java.exe"} else {"java"})); }
    out.push(PathBuf::from(if cfg!(windows) {"java.exe"} else {"java"}));
    #[cfg(windows)] for p in [r"C:\\Program Files\\Java", r"C:\\Program Files\\Eclipse Adoptium", r"C:\\Program Files\\Microsoft"] {
        if let Ok(entries) = std::fs::read_dir(p) { for e in entries.flatten() { out.push(e.path().join("bin/java.exe")); } }
    }
    out
}

pub fn detect() -> Vec<JavaInfo> {
    let mut found = Vec::new();
    for path in candidates() {
        let result = Command::new(&path).arg("-version").output();
        if let Ok(output) = result {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stderr).to_string() + &String::from_utf8_lossy(&output.stdout);
                let version = text.lines().next().unwrap_or("Java version невідома").to_string();
                if !found.iter().any(|x: &JavaInfo| x.executable == path) { found.push(JavaInfo { executable: path, version }); }
            }
        }
    }
    found
}

pub fn verify(path: &str) -> Result<String> {
    let out = Command::new(path).arg("-version").output().with_context(|| format!("Не вдалося запустити Java: {path}"))?;
    if !out.status.success() { anyhow::bail!("Java завершилася з кодом {:?}", out.status.code()); }
    Ok(String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("Java доступна").to_string())
}
