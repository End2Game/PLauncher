use sysinfo::System;

#[derive(Debug, Clone, Copy)]
pub struct Snapshot { pub used_memory_mb: u64, pub global_cpu_percent: f32 }

pub fn snapshot() -> Snapshot {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_usage();
    Snapshot { used_memory_mb: sys.used_memory() / 1024 / 1024, global_cpu_percent: sys.global_cpu_usage() }
}
