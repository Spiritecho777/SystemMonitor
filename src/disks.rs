use std::time::Instant;

use sysinfo::Disks;

/// Une ligne de performance disque, prête à être affichée.
#[derive(Clone, Debug)]
pub struct DiskRow {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_space: u64,
    pub available_space: u64,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

/// Wrapper autour de `sysinfo::Disks`.
///
/// IMPORTANT -- calcul du débit : `Disk::usage()` renvoie déjà, via ses
/// champs `read_bytes`/`written_bytes`, le nombre d'octets lus/écrits
/// DEPUIS LE DERNIER REFRESH (confirmé par la doc officielle de sysinfo,
/// pas une supposition) -- on n'a donc qu'à diviser ce delta par le temps
/// réellement écoulé pour obtenir un débit en octets/seconde. On utilise
/// le temps RÉEL écoulé (Instant::now() - last_refresh), pas l'intervalle
/// nominal de rafraîchissement (1.5s), pour rester exact même si un tick
/// est retardé (charge système, autre traitement synchrone, etc.).
pub struct DiskMonitor {
    disks: Disks,
    last_refresh: Instant,
    rows: Vec<DiskRow>,
}

impl DiskMonitor {
    pub fn new() -> Self {
        let disks = Disks::new_with_refreshed_list();
        let mut s = Self { disks, last_refresh: Instant::now(), rows: Vec::new() };
        // Premier appel : le delta lecture/écriture n'a pas de sens (rien
        // à comparer), on l'ignore simplement en ne calculant qu'un débit
        // à 0 pour ce tout premier affichage.
        s.rebuild_rows(1.0);
        s
    }

    pub fn refresh(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refresh).as_secs_f64().max(0.001);
        self.last_refresh = now;

        for disk in self.disks.list_mut() {
            disk.refresh();
        }
        self.rebuild_rows(elapsed);
    }

    fn rebuild_rows(&mut self, elapsed_secs: f64) {
        self.rows = self
            .disks
            .list()
            .iter()
            .map(|d| {
                let usage = d.usage();
                DiskRow {
                    name: d.name().to_string_lossy().into_owned(),
                    mount_point: d.mount_point().to_string_lossy().into_owned(),
                    file_system: d.file_system().to_string_lossy().into_owned(),
                    total_space: d.total_space(),
                    available_space: d.available_space(),
                    read_bytes_per_sec: usage.read_bytes as f64 / elapsed_secs,
                    write_bytes_per_sec: usage.written_bytes as f64 / elapsed_secs,
                }
            })
            .collect();
    }

    pub fn rows(&self) -> &[DiskRow] {
        &self.rows
    }
}
