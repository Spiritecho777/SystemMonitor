use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use egui_plot::{Legend, Line, Plot, PlotPoints};

use crate::process_monitor::{ProcessMonitor, ProcessRow};
use crate::temperature::TemperatureMonitor;

const REFRESH_INTERVAL: Duration = Duration::from_millis(1500);
const HISTORY_LEN: usize = 120; // ~3 minutes d'historique à 1.5s/tick

#[derive(PartialEq, Clone, Copy)]
enum SortColumn {
    Name,
    Pid,
    Cpu,
    Memory,
}

pub struct TaskManagerApp {
    process_monitor: ProcessMonitor,
    temperature_monitor: TemperatureMonitor,

    processes: Vec<ProcessRow>,
    sort_column: SortColumn,
    sort_desc: bool,
    filter: String,

    // historiques pour les courbes
    cpu_history: VecDeque<f64>,
    temp_history: std::collections::HashMap<String, VecDeque<f64>>,

    last_refresh: Instant,
    pid_to_kill: Option<u32>,
}

impl TaskManagerApp {
    pub fn new() -> Self {
        let mut process_monitor = ProcessMonitor::new();
        let mut temperature_monitor = TemperatureMonitor::new();
        // premier refresh immédiat pour ne pas afficher un état vide au démarrage
        process_monitor.refresh();
        temperature_monitor.refresh();

        Self {
            processes: process_monitor.processes(),
            process_monitor,
            temperature_monitor,
            sort_column: SortColumn::Cpu,
            sort_desc: true,
            filter: String::new(),
            cpu_history: VecDeque::with_capacity(HISTORY_LEN),
            temp_history: std::collections::HashMap::new(),
            last_refresh: Instant::now(),
            pid_to_kill: None,
        }
    }

    fn refresh_if_needed(&mut self) {
        if self.last_refresh.elapsed() < REFRESH_INTERVAL {
            return;
        }
        self.last_refresh = Instant::now();

        self.process_monitor.refresh();
        self.temperature_monitor.refresh();
        self.processes = self.process_monitor.processes();
        self.sort_processes();

        push_capped(&mut self.cpu_history, self.process_monitor.global_cpu_usage() as f64);

        for reading in self.temperature_monitor.readings() {
            let entry = self
                .temp_history
                .entry(reading.label.clone())
                .or_insert_with(|| VecDeque::with_capacity(HISTORY_LEN));
            push_capped(entry, reading.temperature as f64);
        }
    }

    fn sort_processes(&mut self) {
        let desc = self.sort_desc;
        match self.sort_column {
            SortColumn::Name => self.processes.sort_by(|a, b| a.name.cmp(&b.name)),
            SortColumn::Pid => self.processes.sort_by_key(|p| p.pid),
            SortColumn::Cpu => self
                .processes
                .sort_by(|a, b| a.cpu_usage.partial_cmp(&b.cpu_usage).unwrap()),
            SortColumn::Memory => self.processes.sort_by_key(|p| p.memory_bytes),
        }
        if desc {
            self.processes.reverse();
        }
    }

    fn header_button(&mut self, ui: &mut egui::Ui, label: &str, col: SortColumn) {
        let arrow = if self.sort_column == col {
            if self.sort_desc { " ▼" } else { " ▲" }
        } else {
            ""
        };
        if ui.button(format!("{label}{arrow}")).clicked() {
            if self.sort_column == col {
                self.sort_desc = !self.sort_desc;
            } else {
                self.sort_column = col;
                self.sort_desc = true;
            }
            self.sort_processes();
        }
    }
}

fn push_capped(buf: &mut VecDeque<f64>, value: f64) {
    if buf.len() >= HISTORY_LEN {
        buf.pop_front();
    }
    buf.push_back(value);
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    format!("{value:.1} {}", UNITS[unit_idx])
}

impl eframe::App for TaskManagerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.refresh_if_needed();

        // on redemande une frame régulièrement même sans interaction utilisateur,
        // sinon egui ne redessine que sur événement (clic, resize...).
        ctx.request_repaint_after(Duration::from_millis(300));

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("Gestionnaire de tâches");
                ui.separator();
                ui.label(format!(
                    "CPU global : {:.1} % ({} coeurs)",
                    self.process_monitor.global_cpu_usage(),
                    self.process_monitor.cpu_count()
                ));
                ui.separator();
                ui.label(format!(
                    "RAM : {} / {}",
                    human_bytes(self.process_monitor.used_memory()),
                    human_bytes(self.process_monitor.total_memory())
                ));
            });
            ui.add_space(4.0);
        });

        egui::SidePanel::right("sensors_panel")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Température");
                ui.add_space(6.0);

                if !self.temperature_monitor.has_sensors() {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "Aucun capteur détecté.\nSur Windows, cela vient souvent\nde WMI qui n'expose pas de\nthermal zone (dépend du BIOS).",
                    );
                } else {
                    for reading in self.temperature_monitor.readings() {
                        ui.horizontal(|ui| {
                            let color = temp_color(reading.temperature, reading.critical);
                            ui.colored_label(color, format!("{:.1} °C", reading.temperature));
                            ui.label(&reading.label);
                        });
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(6.0);

                    Plot::new("temp_plot")
                        .height(220.0)
                        .legend(Legend::default())
                        .include_y(0.0)
                        .show(ui, |plot_ui| {
                            for (label, history) in &self.temp_history {
                                let points: PlotPoints = history
                                    .iter()
                                    .enumerate()
                                    .map(|(i, v)| [i as f64, *v])
                                    .collect();
                                plot_ui.line(Line::new(points).name(label));
                            }
                        });
                }

                ui.add_space(16.0);
                ui.separator();
                ui.heading("CPU (historique)");
                Plot::new("cpu_plot")
                    .height(160.0)
                    .include_y(0.0)
                    .include_y(100.0)
                    .show(ui, |plot_ui| {
                        let points: PlotPoints = self
                            .cpu_history
                            .iter()
                            .enumerate()
                            .map(|(i, v)| [i as f64, *v])
                            .collect();
                        plot_ui.line(Line::new(points).name("CPU %"));
                    });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Filtre :");
                ui.text_edit_singleline(&mut self.filter);
            });
            ui.add_space(6.0);

            let filter_lower = self.filter.to_lowercase();

            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .column(Column::remainder().at_least(150.0)) // nom
                .column(Column::auto().at_least(70.0)) // pid
                .column(Column::auto().at_least(80.0)) // cpu
                .column(Column::auto().at_least(100.0)) // mémoire
                .column(Column::auto().at_least(90.0)) // statut
                .column(Column::auto().at_least(70.0)) // action
                .header(24.0, |mut header| {
                    header.col(|ui| {
                        self.header_button(ui, "Nom", SortColumn::Name);
                    });
                    header.col(|ui| {
                        self.header_button(ui, "PID", SortColumn::Pid);
                    });
                    header.col(|ui| {
                        self.header_button(ui, "CPU %", SortColumn::Cpu);
                    });
                    header.col(|ui| {
                        self.header_button(ui, "Mémoire", SortColumn::Memory);
                    });
                    header.col(|ui| {
                        ui.label("Statut");
                    });
                    header.col(|ui| {
                        ui.label("");
                    });
                })
                .body(|mut body| {
                    for proc_ in self
                        .processes
                        .iter()
                        .filter(|p| filter_lower.is_empty() || p.name.to_lowercase().contains(&filter_lower))
                    {
                        body.row(22.0, |mut row| {
                            row.col(|ui| {
                                ui.label(&proc_.name);
                            });
                            row.col(|ui| {
                                ui.label(proc_.pid.to_string());
                            });
                            row.col(|ui| {
                                ui.label(format!("{:.1}", proc_.cpu_usage));
                            });
                            row.col(|ui| {
                                ui.label(human_bytes(proc_.memory_bytes));
                            });
                            row.col(|ui| {
                                ui.label(&proc_.status);
                            });
                            row.col(|ui| {
                                if ui.button("Tuer").clicked() {
                                    self.pid_to_kill = Some(proc_.pid);
                                }
                            });
                        });
                    }
                });
        });

        // on traite la demande de kill en dehors de la boucle d'affichage
        // (on ne veut pas muter self.processes pendant qu'on itère dessus)
        if let Some(pid) = self.pid_to_kill.take() {
            self.process_monitor.kill(pid);
            self.processes = self.process_monitor.processes();
        }
    }
}

fn temp_color(temp: f32, critical: Option<f32>) -> egui::Color32 {
    let crit = critical.unwrap_or(85.0);
    if temp >= crit {
        egui::Color32::RED
    } else if temp >= crit * 0.85 {
        egui::Color32::from_rgb(255, 165, 0) // orange
    } else {
        egui::Color32::from_rgb(100, 220, 100) // vert
    }
}
