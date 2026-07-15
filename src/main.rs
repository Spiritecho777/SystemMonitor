mod app;
mod process_monitor;
mod temperature;

use app::TaskManagerApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([700.0, 450.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Gestionnaire de tâches",
        options,
        Box::new(|_cc| Box::new(TaskManagerApp::new())),
    )
}
