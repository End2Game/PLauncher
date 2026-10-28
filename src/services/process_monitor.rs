use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Debug, Clone)]
pub struct MinecraftProcess {
    pub pid: u32,
    pub name: String,
    pub memory_mb: u64,
    pub cpu_percent: f32,
}
pub fn minecraft_processes() -> Vec<MinecraftProcess> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system.processes().iter().filter_map(|(pid, process)| {
        let name = process.name().to_string_lossy().to_string();
        let command = process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ");
        let looks_like_minecraft = name.to_lowercase().contains("java")
            && (command.to_lowercase().contains("net.minecraft")
                || command.to_lowercase().contains("minecraft"));
        if looks_like_minecraft {
            Some(MinecraftProcess {
                pid: pid.as_u32(),
                name,
                memory_mb: process.memory() / 1024 / 1024,
                cpu_percent: process.cpu_usage(),
            })
        } else { None }
    }).collect()
}
