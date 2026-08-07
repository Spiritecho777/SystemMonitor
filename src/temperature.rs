use sysinfo::Components;

#[derive(Clone, Debug)]
pub struct SensorReading {
    pub label: String,
    pub temperature: f32, // en °C
    pub max: Option<f32>,
    pub critical: Option<f32>,
}
pub struct TemperatureMonitor {
    components: Components,
}

impl TemperatureMonitor {
    pub fn new() -> Self {
        let components = Components::new_with_refreshed_list();
        Self { components }
    }

    pub fn refresh(&mut self) {
        self.components = Components::new_with_refreshed_list();
    }

    pub fn readings(&self) -> Vec<SensorReading> {
        self.components
            .iter()
            .filter_map(|c| {
                let temperature = c.temperature()?;
                Some(SensorReading {
                    label: c.label().to_string(),
                    temperature,
                    max: c.max().filter(|v| *v > 0.0),
                    critical: c.critical(),
                })
            })
            .collect()
    }

    pub fn has_sensors(&self) -> bool {
        !self.components.is_empty()
    }
}
