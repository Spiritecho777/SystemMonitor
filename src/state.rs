use std::collections::VecDeque;

use crate::process_monitor::ProcessMonitor;
use crate::services::{ServiceMonitor, ServiceRow};
use crate::temperature::{SensorReading, TemperatureMonitor};

pub const HISTORY_LEN: usize = 120; // ~4 minutes d'historique à 2s/tick

const MIN_SIGNIFICANT_TEMP: f32 = 1.0;

const SERVICE_REFRESH_EVERY_N_TICKS: u32 = 4; // ~6s à 1.5s/tick

pub struct AppState {
    pub process_monitor: ProcessMonitor,
    pub temperature_monitor: TemperatureMonitor,
    pub service_monitor: ServiceMonitor,
    pub cpu_history: VecDeque<f64>,
    pub temp_history: VecDeque<f64>,
    tick_count: u32,
}

impl AppState {
    pub fn new() -> Self {
        let mut process_monitor = ProcessMonitor::new();
        let mut temperature_monitor = TemperatureMonitor::new();
        // ServiceMonitor::new() fait déjà un premier refresh() en interne.
        let service_monitor = ServiceMonitor::new();
        // premier refresh immédiat pour ne pas démarrer sur un état vide
        process_monitor.refresh();
        temperature_monitor.refresh();

        Self {
            process_monitor,
            temperature_monitor,
            service_monitor,
            cpu_history: VecDeque::with_capacity(HISTORY_LEN),
            temp_history: VecDeque::with_capacity(HISTORY_LEN),
            tick_count: 0,
        }
    }

    pub fn refresh(&mut self) {
        self.process_monitor.refresh();
        self.temperature_monitor.refresh();

        self.tick_count = self.tick_count.wrapping_add(1);
        if self.tick_count % SERVICE_REFRESH_EVERY_N_TICKS == 0 {
            self.service_monitor.refresh();
        }

        push_capped(&mut self.cpu_history, self.process_monitor.global_cpu_usage() as f64);

        let readings = self.temperature_monitor.readings();
        if let Some(temp) = primary_temperature(&readings) {
            push_capped(&mut self.temp_history, temp);
        }
    }

    pub fn services(&self) -> &[ServiceRow] {
        self.service_monitor.services()
    }
}

fn primary_temperature(readings: &[SensorReading]) -> Option<f64> {
    let significant: Vec<&SensorReading> = readings
        .iter()
        .filter(|r| r.temperature > MIN_SIGNIFICANT_TEMP)
        .collect();

    if significant.is_empty() {
        return None;
    }

    let cpu_like = significant.iter().find(|r| {
        let l = r.label.to_lowercase();
        l.contains("package") || l.contains("tctl") || l.contains("tdie") || l.contains("cpu")
    });

    let chosen = match cpu_like {
        Some(r) => *r,
        None => significant
            .iter()
            .copied()
            .max_by(|a, b| a.temperature.partial_cmp(&b.temperature).unwrap())
            .expect("significant non vide, vérifié ci-dessus"),
    };

    Some(chosen.temperature as f64)
}

fn push_capped(buf: &mut VecDeque<f64>, value: f64) {
    if buf.len() >= HISTORY_LEN {
        buf.pop_front();
    }
    buf.push_back(value);
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    format!("{value:.1} {}", UNITS[unit_idx])
}
