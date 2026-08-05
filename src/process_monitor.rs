use sysinfo::{MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

#[derive(Clone, Debug)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32, // en %, peut dépasser 100 sur multi-coeurs
    pub memory_bytes: u64,
    pub status: String,
}

pub struct ProcessMonitor {
    sys: System,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        // On ne rafraîchit que ce dont on a besoin, pour rester léger.
        let refresh = RefreshKind::nothing()
            .with_processes(ProcessRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything());
        let mut sys = System::new_with_specifics(refresh);
        sys.refresh_all();
        Self { sys }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_cpu_all();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.sys.refresh_memory();
    }

    pub fn processes(&self) -> Vec<ProcessRow> {
        self.sys
            .processes()
            .iter()
            .map(|(pid, proc_)| ProcessRow {
                pid: pid.as_u32(),
                name: proc_.name().to_string_lossy().into_owned(),
                cpu_usage: proc_.cpu_usage(),
                memory_bytes: proc_.memory(),
                status: proc_.status().to_string(),
            })
            .collect()
    }

    pub fn kill(&self, pid: u32) -> bool {
        if let Some(proc_) = self.sys.process(Pid::from_u32(pid)) {
            proc_.kill()
        } else {
            false
        }
    }

    pub fn total_memory(&self) -> u64 {
        self.sys.total_memory()
    }

    pub fn used_memory(&self) -> u64 {
        self.sys.used_memory()
    }

    pub fn global_cpu_usage(&self) -> f32 {
        self.sys.global_cpu_usage()
    }

    pub fn cpu_count(&self) -> usize {
        self.sys.cpus().len()
    }
}
