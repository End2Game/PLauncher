use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{fs, path::{Component, Path, PathBuf}, process::{Command, Stdio}};

fn allowed_by_rules(rules: Option<&Vec<Value>>) -> bool {
    let Some(rules) = rules else { return true; };
    let os = if cfg!(windows) {"windows"} else if cfg!(target_os="macos") {"osx"} else {"linux"};
    let arch = if cfg!(target_arch="x86_64") {"x86_64"} else if cfg!(target_arch="x86") {"x86"} else { std::env::consts::ARCH };
    let mut allowed = false;
    for rule in rules {
        let os_rule = rule.get("os");
        let os_match = os_rule.map(|o| {
            let name_ok = o.get("name").and_then(Value::as_str).map(|v| v == os).unwrap_or(true);
            let arch_ok = o.get("arch").and_then(Value::as_str).map(|v| v == arch || (v == "x86" && arch == "x86")).unwrap_or(true);
            name_ok && arch_ok
        }).unwrap_or(true);
        let features_match = rule.get("features").and_then(Value::as_object).map(|features| {
            features.iter().all(|(key, expected)| {
                // Launcher-only feature flags are false unless explicitly implemented.
                let actual = matches!(key.as_str(), "has_custom_resolution" | "is_demo_user" | "has_quick_plays_support") && false;
                expected.as_bool().map(|v| v == actual).unwrap_or(false)
            })
        }).unwrap_or(true);
        if os_match && features_match {
            allowed = rule.get("action").and_then(Value::as_str) == Some("allow");
        }
    }
    allowed
}

fn library_path(lib: &Value) -> Option<String> {
    if let Some(path) = lib.pointer("/downloads/artifact/path").and_then(Value::as_str) {
        return Some(path.to_string());
    }
    let coordinate = lib.get("name")?.as_str()?;
    let mut parts = coordinate.split(':');
    let group = parts.next()?.replace('.', "/");
    let artifact = parts.next()?;
    let version = parts.next()?;
    let classifier = parts.next();
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    Some(format!("{group}/{artifact}/{version}/{file}"))
}

fn extract_native_libraries(libraries: Option<&Vec<Value>>, mc: &Path, natives: &Path) -> Result<()> {
    let Some(libraries) = libraries else { return Ok(()); };
    let os = if cfg!(windows) {"windows"} else if cfg!(target_os="macos") {"osx"} else {"linux"};
    let arch = if cfg!(target_pointer_width="64") {"64"} else {"32"};
    for lib in libraries {
        if !allowed_by_rules(lib.get("rules").and_then(Value::as_array)) { continue; }
        let classifier = lib.pointer(&format!("/natives/{os}")).and_then(Value::as_str)
            .map(|v| v.replace("${arch}", arch));
        let Some(classifier) = classifier else { continue; };
        let artifact_path = lib.pointer(&format!("/downloads/classifiers/{classifier}/path"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                let coordinate = lib.get("name")?.as_str()?;
                let mut parts = coordinate.split(':');
                let group = parts.next()?.replace('.', "/");
                let artifact = parts.next()?;
                let version = parts.next()?;
                Some(format!("{group}/{artifact}/{version}/{artifact}-{version}-{classifier}.jar"))
            });
        let Some(relative) = artifact_path else { continue; };
        let archive_path = mc.join("libraries").join(relative);
        if !archive_path.is_file() { continue; }
        let file = fs::File::open(&archive_path)?;
        let mut zip = zip::ZipArchive::new(file)?;
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index)?;
            let name = entry.name().to_string();
            if name.starts_with("META-INF/") { continue; }
            let relative = Path::new(&name);
            if relative.components().any(|c| !matches!(c, Component::Normal(_))) { continue; }
            let destination = natives.join(relative);
            if entry.is_dir() {
                fs::create_dir_all(&destination)?;
            } else {
                if let Some(parent) = destination.parent() { fs::create_dir_all(parent)?; }
                let mut output = fs::File::create(&destination)?;
                std::io::copy(&mut entry, &mut output)?;
            }
        }
    }
    Ok(())
}

fn expand_arg(s: &str, values: &std::collections::HashMap<&str, String>) -> String {
    let mut out = s.to_string();
    for (k,v) in values { out = out.replace(&format!("${{{k}}}"), v); }
    out
}

pub fn launch(instance_dir: &Path, java: &str, username: &str, min_mb: u32, max_mb: u32) -> Result<PathBuf> {
    if username.trim().is_empty() { bail!("Вкажи ім'я гравця для Offline запуску"); }
    let uuid = format!("{:x}", md5::compute(format!("OfflinePlayer:{username}")));
    launch_with_identity(instance_dir, java, username, &uuid, "0", "legacy", min_mb, max_mb)
}

pub fn launch_online(instance_dir: &Path, java: &str, account: &crate::services::account::MinecraftAccount, min_mb: u32, max_mb: u32) -> Result<PathBuf> {
    if account.username.trim().is_empty() || account.uuid.trim().is_empty() || account.access_token.trim().is_empty() {
        bail!("Microsoft-акаунт не містить повного набору даних для запуску");
    }
    launch_with_identity(instance_dir, java, &account.username, &account.uuid, &account.access_token, "msa", min_mb, max_mb)
}

fn launch_with_identity(instance_dir: &Path, java: &str, username: &str, auth_uuid: &str, access_token: &str, user_type: &str, min_mb: u32, max_mb: u32) -> Result<PathBuf> {
    if min_mb < 256 || max_mb < min_mb || max_mb > 65536 { bail!("Некоректні параметри RAM"); }
    let mc = instance_dir.join(".minecraft");
    let descriptor: Value = serde_json::from_slice(&fs::read(instance_dir.join("instance.json"))?)?;
    let version = descriptor.get("minecraft_version").and_then(Value::as_str).context("Немає версії Minecraft в інстансі")?;
    let meta_path = mc.join("versions").join(version).join(format!("{version}.json"));
    let base_meta: Value = serde_json::from_slice(&fs::read(&meta_path).with_context(|| format!("Спочатку встанови Minecraft: {}", meta_path.display()))?)?;
    // Loader profiles inherit the vanilla version. Keep vanilla launch arguments,
    // but add loader libraries and use the loader's main class when present.
    let loader_label = descriptor.get("loader").and_then(Value::as_str).unwrap_or("Vanilla");
    let loader_id = ["Fabric (", "Quilt (", "Forge (", "NeoForge ("].iter()
        .find_map(|prefix| loader_label.strip_prefix(prefix))
        .and_then(|s| s.strip_suffix(')'));
    let loader_meta: Option<Value> = if let Some(id) = loader_id {
        let path = mc.join("versions").join(id).join(format!("{id}.json"));
        let value: Value = serde_json::from_slice(&fs::read(&path)
            .with_context(|| format!("Профіль завантажувача не знайдено: {}", path.display()))?)?;
        if value.get("inheritsFrom").and_then(Value::as_str) != Some(version) {
            bail!("Профіль завантажувача не відповідає версії Minecraft інстансу");
        }
        Some(value)
    } else { None };
    let meta = &base_meta;
    let launch_meta = loader_meta.as_ref().unwrap_or(meta);
    let launch_version_id = loader_id.unwrap_or(version);
    let client = mc.join("versions").join(version).join(format!("{version}.jar"));
    if !client.is_file() { bail!("Не знайдено Minecraft client JAR"); }
    let mut cp = vec![client.to_string_lossy().to_string()];
    if let Some(libs) = meta.get("libraries").and_then(Value::as_array) {
        for lib in libs {
            if !allowed_by_rules(lib.get("rules").and_then(Value::as_array)) { continue; }
            if let Some(path) = library_path(lib) {
                let p = mc.join("libraries").join(path);
                if p.is_file() { cp.push(p.to_string_lossy().to_string()); }
            }
        }
    }
    if let Some(loader) = &loader_meta {
        if let Some(libs) = loader.get("libraries").and_then(Value::as_array) {
            for lib in libs {
                if !allowed_by_rules(lib.get("rules").and_then(Value::as_array)) { continue; }
                if let Some(path) = library_path(lib) {
                    let p = mc.join("libraries").join(path);
                    if p.is_file() && !cp.contains(&p.to_string_lossy().to_string()) {
                        cp.push(p.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    let sep = if cfg!(windows) {";"} else {":"};
    let assets = mc.join("assets");
    let asset_index = meta.pointer("/assetIndex/id").and_then(Value::as_str).unwrap_or(version);
    let uuid = auth_uuid.to_string();
    let natives = mc.join("versions").join(version).join("natives"); fs::create_dir_all(&natives)?;
    // Extract platform native classifiers from both Mojang and loader metadata.
    extract_native_libraries(meta.get("libraries").and_then(Value::as_array), &mc, &natives)?;
    if let Some(loader) = &loader_meta {
        extract_native_libraries(loader.get("libraries").and_then(Value::as_array), &mc, &natives)?;
    }
    let mut vars = std::collections::HashMap::new();
    vars.insert("auth_player_name", username.to_string()); vars.insert("version_name", launch_version_id.to_string());
    vars.insert("game_directory", mc.to_string_lossy().to_string()); vars.insert("assets_root", assets.to_string_lossy().to_string());
    vars.insert("assets_index_name", asset_index.to_string()); vars.insert("auth_uuid", uuid.clone()); vars.insert("auth_access_token", access_token.into());
    vars.insert("user_type", user_type.into()); vars.insert("version_type", meta.get("type").and_then(Value::as_str).unwrap_or("release").into());
    vars.insert("natives_directory", natives.to_string_lossy().to_string()); vars.insert("classpath", cp.join(sep)); vars.insert("classpath_separator", sep.into());
    vars.insert("launcher_name", "PLauncher".into()); vars.insert("launcher_version", "1.7.0".into());
    vars.insert("auth_xuid", String::new()); vars.insert("clientid", String::new());
    vars.insert("auth_session", access_token.to_string()); vars.insert("user_properties", "{}".into()); vars.insert("resolution_width", "854".into()); vars.insert("resolution_height", "480".into());
    vars.insert("quickPlayPath", String::new()); vars.insert("quickPlaySingleplayer", String::new()); vars.insert("quickPlayMultiplayer", String::new()); vars.insert("quickPlayRealms", String::new());
    let mut args: Vec<String> = vec![format!("-Xms{min_mb}M"), format!("-Xmx{max_mb}M")];
    let jvm_arguments = launch_meta.pointer("/arguments/jvm").or_else(|| meta.pointer("/arguments/jvm")).and_then(Value::as_array);
    if let Some(jvm) = jvm_arguments {
        for item in jvm {
            if let Some(s) = item.as_str() { args.push(expand_arg(s, &vars)); }
            else if let Some(rules) = item.get("rules").and_then(Value::as_array) { if allowed_by_rules(Some(rules)) { if let Some(v) = item.get("value") { if let Some(s)=v.as_str(){args.push(expand_arg(s,&vars));} else if let Some(a)=v.as_array(){ for x in a { if let Some(s)=x.as_str(){args.push(expand_arg(s,&vars));} } } } } }
        }
    }
    if !args.iter().any(|a| a.starts_with("-Djava.library.path=")) {
        args.push(format!("-Djava.library.path={}", natives.to_string_lossy()));
    }
    if !args.iter().any(|a| a == "-cp" || a == "-classpath") {
        args.push("-cp".into());
        args.push(cp.join(sep));
    }
    if let Some(game) = launch_meta.pointer("/arguments/game").or_else(|| meta.pointer("/arguments/game")).and_then(Value::as_array) {
        for item in game {
            if let Some(s) = item.as_str() { args.push(expand_arg(s, &vars)); }
            else if let Some(rules) = item.get("rules").and_then(Value::as_array) { if allowed_by_rules(Some(rules)) { if let Some(v) = item.get("value") { if let Some(s)=v.as_str(){args.push(expand_arg(s,&vars));} else if let Some(a)=v.as_array(){ for x in a { if let Some(s)=x.as_str(){args.push(expand_arg(s,&vars));} } } } } }
        }
    } else if let Some(legacy) = launch_meta.get("minecraftArguments").and_then(Value::as_str)
        .or_else(|| meta.get("minecraftArguments").and_then(Value::as_str)) {
        for token in legacy.split_whitespace() { args.push(expand_arg(token, &vars)); }
    } else {
        args.extend(["--username".into(), username.into(), "--version".into(), launch_version_id.into(),
            "--gameDir".into(), mc.to_string_lossy().into(), "--assetsDir".into(), assets.to_string_lossy().into(),
            "--assetIndex".into(), asset_index.into(), "--uuid".into(), uuid, "--accessToken".into(), access_token.into(),
            "--userType".into(), user_type.into()]);
    }
    let main_class = launch_meta.get("mainClass").and_then(Value::as_str)
        .or_else(|| meta.get("mainClass").and_then(Value::as_str))
        .context("У метаданих немає mainClass")?;
    args.push(main_class.into());
    let log_dir = instance_dir.join("logs"); fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join("latest-launch.log");
    let log = fs::File::create(&log_path)?; let err = log.try_clone()?;
    Command::new(java).args(args).current_dir(&mc).stdout(Stdio::from(log)).stderr(Stdio::from(err)).spawn().with_context(|| format!("Не вдалося запустити JVM через {java}"))?;
    Ok(log_path)
}

