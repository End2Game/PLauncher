use eframe::egui;
use std::path::PathBuf;
use crate::{models::GameInstance, services::{doctor, performance, recovery, instances::InstanceService, minecraft::{self, VersionInfo}, java, launch, modrinth, mrpack}};

#[derive(PartialEq, Clone, Copy)]
enum Page { Home, Instances, Mods, Performance, Doctor, Recovery, Settings }
pub struct PLauncherApp {
    page: Page, status: String, game_dir: String, monitor_enabled: bool,
    ram_mb: u64, cpu_percent: f32, diagnostics: Vec<String>,
    instance_root: PathBuf, instances: Vec<GameInstance>, versions: Vec<VersionInfo>,
    new_name: String, selected_version: String, selected_instance: Option<usize>, java_path: String, username: String, min_ram: u32, max_ram: u32, java_found: Vec<java::JavaInfo>, mod_query: String, mod_loader: String, mod_hits: Vec<modrinth::ProjectHit>, selected_mod: Option<usize>, mod_offset: usize, mod_total_hits: usize, installed_mods: Vec<std::path::PathBuf>, oauth_client_id: String, neoforge_version: String, forge_version: String, curseforge_api_key: String, curseforge_query: String, curseforge_hits: Vec<crate::services::curseforge::ModSearchResult>, curseforge_selected: Option<usize>, account_label: String, device_login: Option<crate::services::account::DeviceLogin>,
}
impl PLauncherApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let root = directories::ProjectDirs::from("ua", "PLauncher", "PLauncher").map(|p| p.data_dir().join("instances")).unwrap_or_else(|| PathBuf::from("PLauncherData/instances"));
        let instances = InstanceService::new(root.clone()).and_then(|s| s.load_all()).unwrap_or_default();
        Self { page: Page::Home, status: "Готовий до роботи".into(), game_dir: String::new(), monitor_enabled: true, ram_mb: 0, cpu_percent: 0.0, diagnostics: vec![], instance_root: root, instances, versions: vec![], new_name: String::new(), selected_version: String::new(), selected_instance: None, java_path: "java".into(), username: "Player".into(), min_ram: 1024, max_ram: 4096, java_found: vec![], mod_query: String::new(), mod_loader: "fabric".into(), mod_hits: vec![], selected_mod: None, mod_offset: 0, mod_total_hits: 0, installed_mods: vec![], oauth_client_id: crate::services::settings::load().oauth_client_id, neoforge_version: String::new(), forge_version: String::new(), curseforge_api_key: String::new(), curseforge_query: String::new(), curseforge_hits: vec![], curseforge_selected: None, device_login: None, account_label: crate::services::account::load_account(&crate::services::account::account_path()).map(|x| x.username).unwrap_or_else(|_| "Не авторизовано".into()) }
    }
    fn nav(&mut self, ui: &mut egui::Ui, label: &str, page: Page) { if ui.selectable_label(self.page == page, label).clicked() { self.page = page; } }
    fn refresh_instances(&mut self) { self.instances = InstanceService::new(self.instance_root.clone()).and_then(|s| s.load_all()).unwrap_or_default(); }
    fn instances_page(&mut self, ui: &mut egui::Ui) {
        ui.heading("Інстанси Minecraft");
        ui.label(format!("Каталог: {}", self.instance_root.display()));
        ui.horizontal(|ui| {
            if ui.button("⟳ Завантажити список версій").clicked() {
                self.status = "Отримання офіційного маніфесту Mojang…".into();
                match minecraft::get_versions() { Ok(v) => { self.versions = v; if self.selected_version.is_empty() { self.selected_version = self.versions.iter().find(|x| x.kind == "release").map(|x| x.id.clone()).unwrap_or_default(); } self.status = format!("Знайдено версій: {}", self.versions.len()); }, Err(e) => self.status = format!("Помилка отримання версій: {e}") }
            }
            if ui.button("Оновити інстанси").clicked() { self.refresh_instances(); }
        });
        ui.separator(); ui.label("Створити новий інстанс");
        ui.horizontal(|ui| { ui.label("Назва:"); ui.text_edit_singleline(&mut self.new_name); });
        egui::ComboBox::from_label("Версія Minecraft").selected_text(if self.selected_version.is_empty() { "Спочатку завантаж список" } else { &self.selected_version }).show_ui(ui, |ui| {
            for v in self.versions.iter().filter(|v| v.kind == "release") { ui.selectable_value(&mut self.selected_version, v.id.clone(), format!("{} · {}", v.id, &v.release_time[..10.min(v.release_time.len())])); }
        });
        if ui.add_enabled(!self.new_name.trim().is_empty() && !self.selected_version.is_empty(), egui::Button::new("Створити інстанс")).clicked() {
            match InstanceService::new(self.instance_root.clone()).and_then(|s| s.create(&self.new_name, &self.selected_version)) {
                Ok(item) => { self.status = format!("Створено інстанс {}", item.name); self.new_name.clear(); self.refresh_instances(); }, Err(e) => self.status = format!("Не вдалося створити: {e}")
            }
        }
        ui.separator(); ui.label(format!("Мої інстанси ({})", self.instances.len()));
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (i, item) in self.instances.iter().enumerate() {
                let selected = self.selected_instance == Some(i);
                if ui.selectable_label(selected, format!("{}  ·  Minecraft {}  ·  {}", item.name, item.minecraft_version, if item.installed { "Встановлено" } else { "Не встановлено" })).clicked() { self.selected_instance = Some(i); }
            }
        });
        if let Some(i) = self.selected_instance {
            if let Some(item) = self.instances.get(i).cloned() {
                ui.separator(); ui.label(format!("Обрано: {}", item.name));
                if ui.button("Встановити Vanilla Minecraft").clicked() {
                    let version = self.versions.iter().find(|v| v.id == item.minecraft_version).cloned();
                    if let Some(v) = version {
                        self.status = format!("Встановлення {}…", v.id);
                        match minecraft::install_vanilla(&v, &item.directory, &mut |msg| self.status = msg) {
                            Ok(()) => { let mut updated = item.clone(); updated.installed = true; if let Ok(s) = InstanceService::new(self.instance_root.clone()) { let _ = s.save(&updated); } self.status = format!("{} успішно встановлено", v.id); self.refresh_instances(); },
                            Err(e) => self.status = format!("Помилка встановлення: {e}")
                        }
                    } else { self.status = "Завантаж список версій Mojang перед встановленням".into(); }
                }
                ui.separator();
                ui.heading("Завантажувач Fabric");
                ui.label("Спочатку встанови Vanilla Minecraft цієї версії, потім встанови профіль Fabric.");
                if ui.button("Встановити Fabric").clicked() {
                    self.status = format!("Отримання Fabric для Minecraft {}…", item.minecraft_version);
                    match crate::services::fabric::install_fabric(&item.minecraft_version, &item.directory) {
                        Ok(profile_id) => {
                            let mut updated = item.clone();
                            updated.loader = format!("Fabric ({profile_id})");
                            if let Ok(service) = InstanceService::new(self.instance_root.clone()) {
                                let _ = service.save(&updated);
                            }
                            self.status = format!("Fabric встановлено: {profile_id}. Профіль можна вибрати для запуску.");
                            self.refresh_instances();
                        }
                        Err(e) => self.status = format!("Помилка встановлення Fabric: {e}")
                    }
                }
                ui.separator();
                ui.heading("Завантажувач Quilt");
                ui.label("Спочатку встанови Vanilla Minecraft цієї версії, потім встанови профіль Quilt.");
                if ui.button("Встановити Quilt").clicked() {
                    self.status = format!("Отримання Quilt для Minecraft {}…", item.minecraft_version);
                    match crate::services::quilt::install_quilt(&item.minecraft_version, &item.directory) {
                        Ok(profile_id) => {
                            let mut updated = item.clone();
                            updated.loader = format!("Quilt ({profile_id})");
                            if let Ok(service) = InstanceService::new(self.instance_root.clone()) {
                                let _ = service.save(&updated);
                            }
                            self.status = format!("Quilt встановлено: {profile_id}. Профіль можна вибрати для запуску.");
                            self.refresh_instances();
                        }
                        Err(e) => self.status = format!("Помилка встановлення Quilt: {e}")
                    }
                }
                ui.separator();
                ui.heading("Завантажувач NeoForge");
                ui.horizontal(|ui| {
                    ui.label("Версія NeoForge:");
                    ui.text_edit_singleline(&mut self.neoforge_version);
                    if ui.button("Перевірити сумісність").clicked() {
                        self.status = match crate::services::neoforge::check_compatibility(&item.minecraft_version, &self.neoforge_version) {
                            Ok(message) => message,
                            Err(e) => format!("Несумісна або недоступна версія: {e}")
                        };
                    }
                    if ui.button("Встановити NeoForge").clicked() {
                        self.status = "Завантаження та встановлення NeoForge…".into();
                        let cache = item.directory.join(".minecraft").join("launcher_cache");
                        match crate::services::neoforge::download_installer(&item.minecraft_version, &self.neoforge_version, &cache)
                            .and_then(|jar| crate::services::neoforge::install_client(&self.java_path, &jar, &item.directory.join(".minecraft"))) {
                            Ok(profile_id) => {
                                let mut updated = item.clone();
                                updated.loader = format!("NeoForge ({profile_id})");
                                if let Ok(service) = InstanceService::new(self.instance_root.clone()) { let _ = service.save(&updated); }
                                self.status = format!("NeoForge встановлено: {profile_id}");
                                self.refresh_instances();
                            }
                            Err(e) => self.status = format!("Помилка NeoForge: {e}")
                        }
                    }
                });
                ui.label("Перевірка підтверджує наявність офіційного installer для цієї пари версій, але не гарантує сумісність сторонніх модів.");
                ui.separator();
                ui.heading("Завантажувач Forge");
                ui.horizontal(|ui| {
                    ui.label("Версія Forge:");
                    ui.text_edit_singleline(&mut self.forge_version);
                    if ui.button("Встановити Forge").clicked() {
                        self.status = "Завантаження та встановлення Forge…".into();
                        match crate::services::forge::install_forge(&self.java_path, &item.minecraft_version, &self.forge_version, &item.directory.join(".minecraft")) {
                            Ok(profile_id) => {
                                let mut updated = item.clone();
                                updated.loader = format!("Forge ({profile_id})");
                                if let Ok(service) = InstanceService::new(self.instance_root.clone()) { let _ = service.save(&updated); }
                                self.status = format!("Forge встановлено: {profile_id}");
                                self.refresh_instances();
                            }
                            Err(e) => self.status = format!("Помилка Forge: {e}")
                        }
                    }
                });
                ui.label("Вкажи точну версію Forge для вибраної версії Minecraft (наприклад, 47.3.0 для Minecraft 1.20.1).");
                ui.separator(); ui.heading("Запуск гри · Offline");
                ui.horizontal(|ui| { ui.label("Гравець:"); ui.text_edit_singleline(&mut self.username); });
                ui.horizontal(|ui| { ui.label("Java executable:"); ui.text_edit_singleline(&mut self.java_path); if ui.button("Знайти Java").clicked() { self.java_found = java::detect(); if let Some(j)=self.java_found.first() { self.java_path=j.executable.to_string_lossy().to_string(); } self.status = format!("Знайдено Java: {}", self.java_found.len()); } });
                for j in &self.java_found { ui.label(format!("{} — {}", j.executable.display(), j.version)); }
                ui.horizontal(|ui| { ui.add(egui::Slider::new(&mut self.min_ram, 256..=16384).text("Мін. RAM MB")); ui.add(egui::Slider::new(&mut self.max_ram, 512..=32768).text("Макс. RAM MB")); });
                if ui.button("Перевірити Java").clicked() { self.status = match java::verify(&self.java_path) { Ok(v) => format!("Java готова: {v}"), Err(e) => format!("Java: {e}") }; }
                if ui.add_enabled(item.installed, egui::Button::new("▶ Запустити Minecraft (Offline)" )).clicked() {
                    self.status = match launch::launch(&item.directory, &self.java_path, &self.username, self.min_ram, self.max_ram) { Ok(p) => format!("Minecraft запущено. Лог: {}", p.display()), Err(e) => format!("Помилка запуску: {e}") };
                }
                let saved_account = crate::services::account::load_account(&crate::services::account::account_path()).ok();
                if ui.add_enabled(item.installed && saved_account.is_some(), egui::Button::new("▶ Запустити Minecraft (Microsoft)")).clicked() {
                    if let Some(account) = saved_account {
                        self.status = match launch::launch_online(&item.directory, &self.java_path, &account, self.min_ram, self.max_ram) {
                            Ok(p) => format!("Minecraft запущено через Microsoft. Лог: {}", p.display()),
                            Err(e) => format!("Помилка Microsoft-запуску: {e}")
                        };
                    }
                }
                ui.label("Offline режим не потребує акаунта. Microsoft режим використовує збережений Minecraft access token.");
                ui.label("Увага: запуск online-mode залежить від сумісності аргументів конкретної версії Minecraft і може вимагати додаткового налаштування.");
            }
        }
    }

    fn mods_page(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mod Center · Modrinth");
        let Some(instance) = self.selected_instance.and_then(|i| self.instances.get(i)).cloned() else {
            ui.label("Спочатку обери інстанс у розділі «Інстанси».");
            return;
        };
        let mods_dir = instance.directory.join("mods");
        ui.label(format!("Інстанс: {} · Minecraft {}", instance.name, instance.minecraft_version));
        ui.horizontal(|ui| {
            ui.label("Завантажувач:");
            egui::ComboBox::from_id_source("mod_loader").selected_text(&self.mod_loader).show_ui(ui, |ui| {
                for loader in ["fabric", "forge", "neoforge", "quilt"] { ui.selectable_value(&mut self.mod_loader, loader.to_string(), loader); }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Пошук:");
            ui.text_edit_singleline(&mut self.mod_query);
            if ui.button("Знайти моди").clicked() {
                self.mod_offset = 0;
                self.mod_total_hits = 0;
                self.status = "Пошук у Modrinth…".into();
                match modrinth::search_mods(&self.mod_query, &instance.minecraft_version, &self.mod_loader, 0) {
                    Ok(r) => { self.mod_hits = r.hits; self.mod_total_hits = r.total_hits; self.selected_mod = None; self.status = format!("Знайдено: {} результатів із {}", self.mod_hits.len(), self.mod_total_hits); }
                    Err(e) => self.status = format!("Помилка Modrinth: {e}")
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.add_enabled(self.mod_offset > 0, egui::Button::new("← Попередня")).clicked() {
                self.mod_offset = self.mod_offset.saturating_sub(20);
                match modrinth::search_mods(&self.mod_query, &instance.minecraft_version, &self.mod_loader, self.mod_offset) {
                    Ok(r) => { self.mod_hits = r.hits; self.mod_total_hits = r.total_hits; self.selected_mod = None; self.status = format!("Сторінка завантажена · {} результатів", self.mod_total_hits); }
                    Err(e) => self.status = format!("Помилка Modrinth: {e}")
                }
            }
            ui.label(format!("Результати {}–{} із {}", if self.mod_total_hits == 0 { 0 } else { self.mod_offset + 1 },
                (self.mod_offset + self.mod_hits.len()).min(self.mod_total_hits), self.mod_total_hits));
            if ui.add_enabled(self.mod_offset + 20 < self.mod_total_hits, egui::Button::new("Наступна →")).clicked() {
                self.mod_offset += 20;
                match modrinth::search_mods(&self.mod_query, &instance.minecraft_version, &self.mod_loader, self.mod_offset) {
                    Ok(r) => { self.mod_hits = r.hits; self.mod_total_hits = r.total_hits; self.selected_mod = None; self.status = format!("Сторінка завантажена · {} результатів", self.mod_total_hits); }
                    Err(e) => self.status = format!("Помилка Modrinth: {e}")
                }
            }
        });
        ui.label("Результати пошуку");
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            for (i, hit) in self.mod_hits.iter().enumerate() {
                let label = format!("{} · {} завантажень\n{}", hit.title, hit.downloads, hit.description);
                if ui.selectable_label(self.selected_mod == Some(i), label).clicked() { self.selected_mod = Some(i); }
            }
        });
        if let Some(i) = self.selected_mod {
            if let Some(hit) = self.mod_hits.get(i).cloned() {
                if ui.button(format!("Завантажити «{}» у mods", hit.title)).clicked() {
                    self.status = format!("Завантаження {}…", hit.title);
                    match modrinth::install_latest(&hit.project_id, &instance.minecraft_version, &self.mod_loader, &mods_dir) {
                        Ok(path) => self.status = format!("Мод встановлено: {}", path.display()),
                        Err(e) => self.status = format!("Не вдалося встановити мод: {e}")
                    }
                }
            }
        }
        ui.separator();
        ui.heading("Імпорт Modrinth .mrpack");
        ui.label("Вкажи повний шлях до .mrpack. Файли пакета перевіряються за SHA-1/SHA-512 перед записом.");
        ui.horizontal(|ui| {
            ui.label("Файл:");
            ui.text_edit_singleline(&mut self.game_dir);
            if ui.button("Перевірити пакет").clicked() {
                match mrpack::inspect_mrpack(std::path::Path::new(&self.game_dir)) {
                    Ok(info) => self.status = format!("Пакет: {} · Minecraft {} · {} файлів · залежності: {:?}",
                        info.name.unwrap_or_else(|| "Без назви".into()), info.dependencies.get("minecraft").cloned().unwrap_or_default(),
                        info.files.len(), info.dependencies),
                    Err(e) => self.status = format!("Помилка .mrpack: {e}")
                }
            }
            if ui.button("Імпортувати").clicked() {
                match mrpack::import_mrpack(std::path::Path::new(&self.game_dir), &instance.directory) {
                    Ok(report) => self.status = format!("Імпортовано {} файлів у {}. Minecraft {}, loader {}. Пропущено {}.",
                        report.installed_files, report.name, report.minecraft_version, report.loader, report.skipped_files),
                    Err(e) => self.status = format!("Помилка імпорту: {e}")
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Встановлені моди");
            if ui.button("Оновити список").clicked() {
                self.installed_mods = modrinth::list_installed(&mods_dir).unwrap_or_default();
            }
        });
        if self.installed_mods.is_empty() { ui.label("Немає знайдених JAR-файлів або список ще не оновлено."); }
        let mut remove: Option<String> = None;
        for file in &self.installed_mods {
            ui.horizontal(|ui| {
                ui.label(file.file_name().unwrap_or_default().to_string_lossy());
                if ui.button("Видалити").clicked() { remove = Some(file.file_name().unwrap_or_default().to_string_lossy().to_string()); }
            });
        }
        if let Some(name) = remove {
            match modrinth::remove_mod(&mods_dir, &name) {
                Ok(()) => { self.installed_mods = modrinth::list_installed(&mods_dir).unwrap_or_default(); self.status = format!("Видалено {name}"); },
                Err(e) => self.status = format!("Помилка видалення: {e}")
            }
        }
        ui.separator();
        ui.heading("CurseForge · пошук і встановлення");
        ui.label("Потрібен власний API key CurseForge. Він зберігається лише в пам'яті поточного сеансу.");
        ui.horizontal(|ui| { ui.label("API key:"); ui.add(egui::TextEdit::singleline(&mut self.curseforge_api_key).password(true)); });
        ui.horizontal(|ui| {
            ui.label("Пошук:");
            ui.text_edit_singleline(&mut self.curseforge_query);
            if ui.button("Знайти на CurseForge").clicked() {
                let loader_id = match self.mod_loader.as_str() { "forge" => 1, "fabric" => 4, "quilt" => 5, "neoforge" => 6, _ => 1 };
                match crate::services::curseforge::search_mods(&self.curseforge_api_key, &instance.minecraft_version, loader_id, &self.curseforge_query) {
                    Ok(hits) => { self.curseforge_hits = hits; self.curseforge_selected = None; self.status = format!("CurseForge: знайдено {} проєктів", self.curseforge_hits.len()); }
                    Err(e) => self.status = format!("Помилка CurseForge: {e}")
                }
            }
        });
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            for (i, hit) in self.curseforge_hits.iter().enumerate() {
                if ui.selectable_label(self.curseforge_selected == Some(i), format!("{} · {} завантажень\n{}", hit.name, hit.download_count, hit.summary)).clicked() {
                    self.curseforge_selected = Some(i);
                }
            }
        });
        if let Some(i) = self.curseforge_selected {
            if let Some(hit) = self.curseforge_hits.get(i).cloned() {
                if ui.button(format!("Встановити останній сумісний файл: {}", hit.name)).clicked() {
                    let loader_id = match self.mod_loader.as_str() { "forge" => 1, "fabric" => 4, "quilt" => 5, "neoforge" => 6, _ => 1 };
                    match crate::services::curseforge::install_selected_with_dependencies(&self.curseforge_api_key, hit.id, &instance.minecraft_version, loader_id, &mods_dir) {
                        Ok(paths) => self.status = format!("CurseForge: встановлено {} файл(ів), включно з обов'язковими залежностями.", paths.len()),
                        Err(e) => self.status = format!("Помилка встановлення CurseForge: {e}")
                    }
                }
            }
        }
        ui.label("Обов'язкові залежності CurseForge встановлюються рекурсивно. Необов'язкові та несумісні залежності не додаються автоматично.");
        ui.label("Сумісність Modrinth фільтрується за версією Minecraft і loader. Для коректної роботи модів сам завантажувач має бути встановлений в інстансі.");
    }
}
impl eframe::App for PLauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("sidebar").resizable(false).default_width(205.0).show(ctx, |ui| {
            ui.add_space(14.0); ui.heading("PLauncher"); ui.label(egui::RichText::new("RUST EDITION · 1.3.0").small().weak()); ui.add_space(24.0);
            self.nav(ui, "⌂  Головна", Page::Home); self.nav(ui, "▣  Інстанси", Page::Instances); self.nav(ui, "◇  Mod Center", Page::Mods); ui.add_space(12.0); ui.separator();
            self.nav(ui, "◴  Performance", Page::Performance); self.nav(ui, "⚕  Doctor", Page::Doctor); self.nav(ui, "↻  Recovery", Page::Recovery); ui.add_space(12.0); self.nav(ui, "⚙  Налаштування", Page::Settings);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| { ui.label(egui::RichText::new(&self.status).small().weak()); });
        });
        egui::CentralPanel::default().show(ctx, |ui| { ui.add_space(12.0); match self.page {
            Page::Home => { ui.heading("Вітаємо у PLauncher"); ui.label("Rust + egui · нативний Minecraft Launcher"); ui.add_space(18.0); ui.horizontal_wrapped(|ui| { ui.group(|ui| { ui.label("Ігрові інстанси"); ui.heading(self.instances.len().to_string()); ui.label("локальних профілів"); }); ui.group(|ui| { ui.label("Моніторинг системи"); ui.heading(format!("{} MB", self.ram_mb)); ui.label(format!("CPU: {:.1}%", self.cpu_percent)); }); }); ui.add_space(16.0); ui.label("Minecraft Core: інстанси, встановлення Vanilla, пошук Java та базовий Offline-запуск."); }
            Page::Instances => self.instances_page(ui),
            Page::Mods => self.mods_page(ui),
            Page::Performance => { ui.heading("PLauncher Performance"); ui.checkbox(&mut self.monitor_enabled, "Увімкнути моніторинг ресурсів"); if self.monitor_enabled { let m = performance::snapshot(); self.ram_mb = m.used_memory_mb; self.cpu_percent = m.global_cpu_percent; ui.label(format!("Загальна RAM: {} MB", m.used_memory_mb)); ui.label(format!("Завантаження CPU: {:.1}%", m.global_cpu_percent)); } else { ui.label("Моніторинг вимкнено"); } if ui.button("Оновити показники").clicked() { ctx.request_repaint(); }
                ui.separator();
                ui.heading("Процеси Minecraft");
                let processes = crate::services::process_monitor::minecraft_processes();
                if processes.is_empty() { ui.label("Активних процесів Minecraft не знайдено."); }
                for process in processes {
                    ui.label(format!("PID {} · {} · RAM {} MB · CPU {:.1}%",
                        process.pid, process.name, process.memory_mb, process.cpu_percent));
                }
            }
            Page::Doctor => { ui.heading("PLauncher Doctor"); if ui.button("Запустити діагностику").clicked() { self.diagnostics = doctor::run_basic_checks(); self.status = "Діагностику завершено".into(); } for line in &self.diagnostics { ui.label(line); } }
            Page::Recovery => { ui.heading("PLauncher Recovery"); ui.label("Створення резервної копії каталогу інстансу."); ui.horizontal(|ui| { ui.label("Шлях інстансу:"); ui.text_edit_singleline(&mut self.game_dir); }); if ui.button("Створити ZIP-копію").clicked() { self.status = match recovery::backup_directory(&self.game_dir) { Ok(p) => format!("Резервну копію створено: {}", p.display()), Err(e) => format!("Помилка резервного копіювання: {e}") }; } ui.label("Безпечне відновлення з архіву ще в розробці."); }
            Page::Settings => {
                ui.heading("Акаунт Minecraft");
                ui.label(format!("Статус: {}", self.account_label));
                ui.label("Microsoft OAuth Client ID (потрібна реєстрація Desktop/Public client у Microsoft Entra).");
                ui.text_edit_singleline(&mut self.oauth_client_id);
                if ui.button("Зберегти Client ID").clicked() {
                    let settings = crate::services::settings::AppSettings {
                        oauth_client_id: self.oauth_client_id.trim().to_string(),
                    };
                    self.status = match crate::services::settings::save(&settings) {
                        Ok(()) => "Client ID збережено в налаштуваннях".into(),
                        Err(e) => format!("Не вдалося зберегти налаштування: {e}"),
                    };
                }
                if ui.button("Отримати код входу Microsoft").clicked() {
                    match crate::services::account::begin_device_login(&self.oauth_client_id) {
                        Ok(device) => {
                            self.status = "Відкрий сторінку Microsoft, введи код і підтвердь вхід.".into();
                            self.device_login = Some(device);
                        }
                        Err(e) => self.status = format!("Не вдалося почати вхід: {e}")
                    }
                }
                if let Some(device) = self.device_login.clone() {
                    ui.separator();
                    ui.label("1. Відкрий адресу нижче у браузері.");
                    ui.hyperlink(&device.verification_uri);
                    ui.label("2. Введи цей одноразовий код:");
                    ui.horizontal(|ui| {
                        ui.heading(device.user_code.clone());
                        if ui.button("Копіювати код").clicked() {
                            ui.ctx().copy_text(device.user_code.clone());
                            self.status = "Код скопійовано".into();
                        }
                    });
                    ui.label(format!("Термін дії: приблизно {} секунд", device.expires_in));
                    if ui.button("Я підтвердив вхід — завершити авторизацію").clicked() {
                        self.status = "Перевірка підтвердження Microsoft…".into();
                        match crate::services::account::complete_device_login(&self.oauth_client_id, &device) {
                            Ok(account) => {
                                let name = account.username.clone();
                                match crate::services::account::save_account(&crate::services::account::account_path(), &account) {
                                    Ok(()) => {
                                        self.account_label = name;
                                        self.device_login = None;
                                        self.status = "Microsoft-профіль підключено. Доступний експериментальний онлайн-запуск; сумісність залежить від версії Minecraft.".into();
                                    }
                                    Err(e) => self.status = format!("Не вдалося зберегти акаунт: {e}")
                                }
                            }
                            Err(e) => {
                                self.device_login = None;
                                self.status = format!("Помилка авторизації: {e}")
                            }
                        }
                    }
                    if ui.button("Скасувати вхід").clicked() {
                        self.device_login = None;
                        self.status = "Вхід скасовано".into();
                    }
                }
                if ui.button("Вийти з акаунта").clicked() {
                    let path = crate::services::account::account_path();
                    match std::fs::remove_file(path) {
                        Ok(()) => { self.account_label = "Не авторизовано".into(); self.status = "Акаунт від'єднано".into(); },
                        Err(e) => self.status = format!("Не вдалося видалити локальну сесію: {e}")
                    }
                }
                ui.separator();
 ui.heading("Налаштування"); ui.label("Конфігурація та збереження параметрів — наступний етап."); }
        }});
        ctx.request_repaint_after(std::time::Duration::from_secs(2));
    }
}
