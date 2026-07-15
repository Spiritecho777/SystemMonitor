use sysinfo::Components;

/// Une sonde de température (CPU, GPU, chipset, disque...).
#[derive(Clone, Debug)]
pub struct SensorReading {
    pub label: String,
    pub temperature: f32, // en °C
    pub max: Option<f32>,
    pub critical: Option<f32>,
}

/// Wrapper autour de `sysinfo::Components`, qui lit :
/// - Linux : /sys/class/hwmon/*
/// - Windows : MSAcpi_ThermalZoneTemperature via WMI (souvent limité/absent
///   selon le BIOS -- voir la note dans le README pour un fallback
///   LibreHardwareMonitor si tu as besoin de plus de précision)
pub struct TemperatureMonitor {
    components: Components,
}

impl TemperatureMonitor {
    pub fn new() -> Self {
        let components = Components::new_with_refreshed_list();
        Self { components }
    }

    pub fn refresh(&mut self) {
        self.components.refresh();
    }

    pub fn readings(&self) -> Vec<SensorReading> {
        self.components
            .iter()
            .map(|c| SensorReading {
                label: c.label().to_string(),
                temperature: c.temperature(),
                max: Some(c.max()).filter(|v| *v > 0.0),
                critical: c.critical(),
            })
            .collect()
    }

    /// Pas de capteur trouvé = probablement un Windows sans WMI thermal zone
    /// exposée, ou une VM. On le signale explicitement dans l'UI plutôt que
    /// d'afficher un tableau vide sans explication.
    pub fn has_sensors(&self) -> bool {
        !self.components.is_empty()
    }
}
