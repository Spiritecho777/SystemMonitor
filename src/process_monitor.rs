use sysinfo::{MemoryRefreshKind, Pid, ProcessRefreshKind, RefreshKind, System};

/// Une ligne de la table de process, prête à être affichée.
#[derive(Clone, Debug)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32, // en %, peut dépasser 100 sur multi-coeurs
    pub memory_bytes: u64,
    pub status: String,
}

/// Encapsule le `System` de sysinfo et expose des données déjà formatées.
pub struct ProcessMonitor {
    sys: System,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        // On ne rafraîchit que ce dont on a besoin, pour rester léger.
        let refresh = RefreshKind::new()
            .with_processes(ProcessRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything());
        let mut sys = System::new_with_specifics(refresh);
        sys.refresh_all();
        Self { sys }
    }

    /// À appeler périodiquement (ex: toutes les 1-2s).
    /// sysinfo a besoin de deux refresh espacés dans le temps pour calculer
    /// un %CPU correct par process (delta entre deux mesures).
    pub fn refresh(&mut self) {
        self.sys.refresh_cpu();
        self.sys.refresh_processes();
        self.sys.refresh_memory();
    }

    pub fn processes(&self) -> Vec<ProcessRow> {
        self.sys
            .processes()
            .iter()
            .map(|(pid, proc_)| ProcessRow {
                pid: pid.as_u32(),
                name: proc_.name().to_string(),
                cpu_usage: proc_.cpu_usage(),
                memory_bytes: proc_.memory(),
                status: proc_.status().to_string(),
            })
            .collect()
    }

    /// Tue le process `pid`. Grace a la capability CAP_KILL attachee au
    /// binaire via `setcap` a l'installation (voir install.sh), ce kill
    /// fonctionne aussi bien pour nos propres process que pour ceux
    /// d'autres utilisateurs -- pas besoin de demon ni de logique
    /// d'escalade separee, sysinfo appelle kill(2) en interne et le
    /// kernel autorise l'appel des lors que le processus courant a
    /// CAP_KILL en effectif.
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
        self.sys.global_cpu_info().cpu_usage()
    }

    pub fn cpu_count(&self) -> usize {
        self.sys.cpus().len()
    }
}
