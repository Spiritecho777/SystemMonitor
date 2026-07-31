use std::collections::{HashMap, VecDeque};

use crate::process_monitor::ProcessMonitor;
use crate::temperature::TemperatureMonitor;

pub const HISTORY_LEN: usize = 120; // ~4 minutes d'historique à 2s/tick

pub struct AppState {
    pub process_monitor: ProcessMonitor,
    pub temperature_monitor: TemperatureMonitor,
    pub cpu_history: VecDeque<f64>,
    pub temp_history: HashMap<String, VecDeque<f64>>,
}

impl AppState {
    pub fn new() -> Self {
        let mut process_monitor = ProcessMonitor::new();
        let mut temperature_monitor = TemperatureMonitor::new();
        // premier refresh immédiat pour ne pas démarrer sur un état vide
        process_monitor.refresh();
        temperature_monitor.refresh();

        Self {
            process_monitor,
            temperature_monitor,
            cpu_history: VecDeque::with_capacity(HISTORY_LEN),
            temp_history: HashMap::new(),
        }
    }

    pub fn refresh(&mut self) {
        self.process_monitor.refresh();
        self.temperature_monitor.refresh();

        push_capped(&mut self.cpu_history, self.process_monitor.global_cpu_usage() as f64);

        for reading in self.temperature_monitor.readings() {
            let entry = self
                .temp_history
                .entry(reading.label.clone())
                .or_insert_with(|| VecDeque::with_capacity(HISTORY_LEN));
            push_capped(entry, reading.temperature as f64);
        }
    }
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
