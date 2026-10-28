use std::process::Command;

pub fn run_basic_checks() -> Vec<String> {
    let mut out = vec!["PLauncher Doctor — базова діагностика".to_string()];
    match Command::new("java").arg("-version").output() {
        Ok(result) => { let s = String::from_utf8_lossy(&result.stderr); out.push(format!("Java: знайдено ({})", s.lines().next().unwrap_or("версія невідома"))); }
        Err(_) => out.push("Java: не знайдено у PATH".into()),
    }
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) { out.push(format!("Домашній каталог: {}", std::path::PathBuf::from(home).display())); }
    out.push(format!("ОС: {}", std::env::consts::OS));
    out.push(format!("Архітектура: {}", std::env::consts::ARCH));
    out.push("Примітка: перевірка поки не аналізує Minecraft libraries або loader.".into());
    out
}
