mod process_monitor;
mod state;
mod temperature;

use std::cell::RefCell;
use std::rc::Rc;

use fltk::{
    app,
    button::Button,
    draw,
    enums::{Align, Color, Event, Font, FrameType},
    frame::Frame,
    group::{Flex, Pack, PackType, Scroll},
    input::Input,
    prelude::*,
    table::{Table, TableContext},
    window::Window,
};

use process_monitor::ProcessRow;
use state::{human_bytes, AppState, HISTORY_LEN};
use temperature::SensorReading;

const REFRESH_MS: i32 = 1500;
const ROW_HEIGHT: i32 = 24;

#[derive(Clone, Copy, PartialEq)]
enum SortKey {
    Name,
    Pid,
    Cpu,
    Memory,
    Status,
}

const COLUMN_TITLES: [&str; 5] = ["Nom", "PID", "CPU %", "Mémoire", "Statut"];
const COLUMN_KEYS: [SortKey; 5] = [SortKey::Name, SortKey::Pid, SortKey::Cpu, SortKey::Memory, SortKey::Status];

/// Tout ce qui doit survivre entre les callbacks (state applicatif + widgets
/// à rafraîchir + état d'UI transitoire comme le tri/le filtre/la sélection).
struct Shared {
    app_state: AppState,
    filtered_rows: Vec<ProcessRow>,
    sort_key: SortKey,
    sort_desc: bool,
    filter_text: String,
    selected_pid: Option<u32>,
}

impl Shared {
    fn recompute_rows(&mut self) {
        let needle = self.filter_text.to_lowercase();
        let mut rows: Vec<ProcessRow> = self
            .app_state
            .process_monitor
            .processes()
            .into_iter()
            .filter(|p| needle.is_empty() || p.name.to_lowercase().contains(&needle))
            .collect();

        rows.sort_by(|a, b| {
            let ord = match self.sort_key {
                SortKey::Name => a.name.cmp(&b.name),
                SortKey::Pid => a.pid.cmp(&b.pid),
                SortKey::Cpu => a.cpu_usage.partial_cmp(&b.cpu_usage).unwrap_or(std::cmp::Ordering::Equal),
                SortKey::Memory => a.memory_bytes.cmp(&b.memory_bytes),
                SortKey::Status => a.status.cmp(&b.status),
            };
            if self.sort_desc {
                ord.reverse()
            } else {
                ord
            }
        });

        self.filtered_rows = rows;
    }
}

fn main() {
    let app = app::App::default();
    app::set_visible_focus(false);

    let mut window = Window::default().with_size(1150, 700).with_label("Gestionnaire de tâches");

    let mut root = Flex::default_fill().column();

    // --- barre du haut : CPU / RAM ---
    let mut top_bar = Flex::default().row();
    top_bar.set_frame(FrameType::FlatBox);
    let mut cpu_label = Frame::default().with_label("CPU : --");
    cpu_label.set_align(Align::Left | Align::Inside);
    let mut ram_label = Frame::default().with_label("RAM : --");
    ram_label.set_align(Align::Left | Align::Inside);
    top_bar.end();
    root.fixed(&top_bar, 28);

    // --- corps : table (gauche) + panneau température (droite) ---
    let mut body = Flex::default().row();

    // ===== gauche =====
    let mut left = Flex::default().column();

    let mut filter_input = Input::default();
    filter_input.set_tooltip("Filtrer par nom de process");
    left.fixed(&filter_input, 28);

    let header = Flex::default().row();
    let mut header_labels: Vec<Frame> = Vec::new();
    for title in COLUMN_TITLES {
        let mut lbl = Frame::default().with_label(title);
        lbl.set_frame(FrameType::UpBox);
        lbl.set_label_font(Font::HelveticaBold);
        header_labels.push(lbl);
    }
    header.end();
    left.fixed(&header, 22);

    let mut table = Table::default();
    table.set_rows(0);
    table.set_row_header(false);
    table.set_cols(COLUMN_TITLES.len() as i32);
    table.set_col_header(false); // en-tête géré à la main juste au-dessus (colonnes cliquables pour le tri)
    table.set_row_height_all(ROW_HEIGHT);
    table.set_col_resize(true);
    table.end();

    left.end();

    // séparateur visuel
    let mut sep = Frame::default();
    sep.set_frame(FrameType::ThinDownFrame);
    body.fixed(&sep, 2);

    // ===== droite : température =====
    let mut right = Flex::default().column();
    body.fixed(&right, 340);

    let mut temp_heading = Frame::default().with_label("Température");
    temp_heading.set_label_font(Font::HelveticaBold);
    temp_heading.set_align(Align::Left | Align::Inside);
    right.fixed(&temp_heading, 24);

    let sensors_scroll = Scroll::default();
    let mut sensors_pack = Pack::default();
    sensors_pack.set_type(PackType::Vertical);
    sensors_pack.set_spacing(2);
    sensors_pack.end();
    sensors_scroll.end();
    right.fixed(&sensors_scroll, 140);

    let mut temp_plot_heading = Frame::default().with_label("Historique température");
    temp_plot_heading.set_label_font(Font::HelveticaBold);
    temp_plot_heading.set_align(Align::Left | Align::Inside);
    right.fixed(&temp_plot_heading, 22);

    let mut temp_plot = Frame::default();
    temp_plot.set_frame(FrameType::DownBox);
    right.fixed(&temp_plot, 160);

    let mut cpu_plot_heading = Frame::default().with_label("Historique CPU");
    cpu_plot_heading.set_label_font(Font::HelveticaBold);
    cpu_plot_heading.set_align(Align::Left | Align::Inside);
    right.fixed(&cpu_plot_heading, 22);

    let mut cpu_plot = Frame::default();
    cpu_plot.set_frame(FrameType::DownBox);
    right.fixed(&cpu_plot, 140);

    let mut kill_button = Button::default().with_label("Tuer le process sélectionné");
    right.fixed(&kill_button, 30);

    let mut selection_label = Frame::default().with_label("Aucune sélection");
    selection_label.set_align(Align::Left | Align::Inside);
    right.fixed(&selection_label, 22);

    right.end();
    body.end();

    root.end();
    window.end();
    window.make_resizable(true);
    window.show();

    // --- état partagé ---
    let shared = Rc::new(RefCell::new(Shared {
        app_state: AppState::new(),
        filtered_rows: Vec::new(),
        sort_key: SortKey::Cpu,
        sort_desc: true,
        filter_text: String::new(),
        selected_pid: None,
    }));
    shared.borrow_mut().recompute_rows();

    // --- dessin des cellules de la table ---
    {
        let shared = shared.clone();
        table.draw_cell(move |t, ctx, row, col, x, y, w, h| match ctx {
            TableContext::Cell => {
                let sh = shared.borrow();
                let is_selected = sh
                    .filtered_rows
                    .get(row as usize)
                    .map(|r| Some(r.pid) == sh.selected_pid)
                    .unwrap_or(false);

                draw::push_clip(x, y, w, h);
                draw::set_draw_color(if is_selected { Color::Selection } else { Color::Background2 });
                draw::draw_rectf(x, y, w, h);

                if let Some(r) = sh.filtered_rows.get(row as usize) {
                    let text = match col {
                        0 => r.name.clone(),
                        1 => r.pid.to_string(),
                        2 => format!("{:.1}", r.cpu_usage),
                        3 => human_bytes(r.memory_bytes),
                        4 => r.status.clone(),
                        _ => String::new(),
                    };
                    draw::set_draw_color(if is_selected { Color::White } else { Color::Foreground });
                    draw::set_font(Font::Helvetica, 13);
                    draw::draw_text2(&text, x + 4, y, w - 4, h, Align::Left);
                }

                draw::set_draw_color(Color::Light2);
                draw::draw_rect(x, y, w, h);
                draw::pop_clip();
                let _ = t;
            }
            _ => {}
        });
    }

    // clic sur une ligne -> sélection (pour le bouton "Tuer")
    {
        let shared = shared.clone();
        let mut selection_label = selection_label.clone();
        table.handle(move |t, ev| {
            if ev == Event::Push {
                let (row, _col) = (t.callback_row(), t.callback_col());
                if row >= 0 {
                    let mut sh = shared.borrow_mut();
                    if let Some(r) = sh.filtered_rows.get(row as usize) {
                        let (pid, name) = (r.pid, r.name.clone());
                        sh.selected_pid = Some(pid);
                        selection_label.set_label(&format!("Sélection : PID {} ({})", pid, name));
                    }
                    t.redraw();
                    return true;
                }
            }
            false
        });
    }

    // clic sur un en-tête de colonne -> tri
    for (i, mut lbl) in header_labels.into_iter().enumerate() {
        let shared = shared.clone();
        let mut table_clone = table.clone();
        lbl.handle(move |_widget, ev| {
            if ev == Event::Push {
                let mut sh = shared.borrow_mut();
                let key = COLUMN_KEYS[i];
                if sh.sort_key == key {
                    sh.sort_desc = !sh.sort_desc;
                } else {
                    sh.sort_key = key;
                    sh.sort_desc = true;
                }
                sh.recompute_rows();
                table_clone.set_rows(sh.filtered_rows.len() as i32);
                table_clone.redraw();
                return true;
            }
            false
        });
    }

    // filtre texte
    {
        let shared = shared.clone();
        let mut table_clone = table.clone();
        filter_input.set_trigger(fltk::enums::CallbackTrigger::Changed);
        filter_input.set_callback(move |inp| {
            let mut sh = shared.borrow_mut();
            sh.filter_text = inp.value();
            sh.recompute_rows();
            table_clone.set_rows(sh.filtered_rows.len() as i32);
            table_clone.redraw();
        });
    }

    // bouton "Tuer"
    {
        let shared = shared.clone();
        kill_button.set_callback(move |_| {
            let mut sh = shared.borrow_mut();
            if let Some(pid) = sh.selected_pid.take() {
                sh.app_state.process_monitor.kill(pid);
            }
        });
    }

    // --- graphes (dessin Cairo-like via le module `draw` de FLTK) ---
    {
        let shared = shared.clone();
        cpu_plot.draw(move |f| {
            let sh = shared.borrow();
            draw_history(f.x(), f.y(), f.w(), f.h(), sh.app_state.cpu_history.iter().copied(), 0.0, 100.0);
        });
    }
    {
        let shared = shared.clone();
        temp_plot.draw(move |f| {
            let sh = shared.borrow();
            let max_temp = sh
                .app_state
                .temp_history
                .values()
                .flat_map(|h| h.iter().copied())
                .fold(60.0_f64, f64::max);
            for history in sh.app_state.temp_history.values() {
                draw_history(f.x(), f.y(), f.w(), f.h(), history.iter().copied(), 0.0, max_temp);
            }
        });
    }

    refresh_widgets(&shared, &mut cpu_label, &mut ram_label, &mut table, &mut sensors_pack, &sensors_scroll);

    // --- boucle de rafraîchissement périodique ---
    let (sender, receiver) = app::channel::<()>();
    app::add_timeout3(REFRESH_MS as f64 / 1000.0, {
        let sender = sender.clone();
        move |handle| {
            sender.send(());
            app::repeat_timeout3(REFRESH_MS as f64 / 1000.0, handle);
        }
    });

    while app.wait() {
        if receiver.recv().is_some() {
            {
                let mut sh = shared.borrow_mut();
                sh.app_state.refresh();
                sh.recompute_rows();
            }
            refresh_widgets(&shared, &mut cpu_label, &mut ram_label, &mut table, &mut sensors_pack, &sensors_scroll);
        }
    }
}

fn refresh_widgets(
    shared: &Rc<RefCell<Shared>>,
    cpu_label: &mut Frame,
    ram_label: &mut Frame,
    table: &mut Table,
    sensors_pack: &mut Pack,
    sensors_scroll: &Scroll,
) {
    let sh = shared.borrow();

    cpu_label.set_label(&format!(
        "CPU : {:.1} % ({} coeurs)",
        sh.app_state.process_monitor.global_cpu_usage(),
        sh.app_state.process_monitor.cpu_count()
    ));
    ram_label.set_label(&format!(
        "RAM : {} / {}",
        human_bytes(sh.app_state.process_monitor.used_memory()),
        human_bytes(sh.app_state.process_monitor.total_memory())
    ));

    table.set_rows(sh.filtered_rows.len() as i32);
    table.redraw();

    // panneau capteurs : on repeuple le Pack à chaque refresh (simple et
    // largement suffisant vu le faible nombre de sondes habituel)
    sensors_pack.clear();
    let readings = sh.app_state.temperature_monitor.readings();
    if !sh.app_state.temperature_monitor.has_sensors() {
        let mut lbl = Frame::default()
            .with_size(sensors_scroll.w() - 20, 60)
            .with_label("Aucun capteur détecté.\nSur Windows, cela vient souvent de WMI\n(dépend du BIOS/carte mère).");
        lbl.set_align(Align::Left | Align::Inside | Align::Wrap);
        sensors_pack.add(&lbl);
    } else {
        for reading in &readings {
            sensors_pack.add(&sensor_row(reading, sensors_scroll.w() - 20));
        }
    }
    sensors_pack.redraw();

    drop(sh);
}

fn sensor_row(reading: &SensorReading, width: i32) -> Frame {
    let crit = reading.critical.unwrap_or(85.0);
    let color = if reading.temperature >= crit {
        Color::from_rgb(0xe7, 0x4c, 0x3c)
    } else if reading.temperature >= crit * 0.85 {
        Color::from_rgb(0xe6, 0x7e, 0x22)
    } else {
        Color::from_rgb(0x2e, 0xcc, 0x71)
    };
    let mut frame = Frame::default()
        .with_size(width, 20)
        .with_label(&format!("{:.1} °C  —  {}", reading.temperature, reading.label));
    frame.set_align(Align::Left | Align::Inside);
    frame.set_label_color(color);
    frame.set_label_font(Font::HelveticaBold);
    frame
}

/// Dessine une courbe simple à partir d'un historique de valeurs.
fn draw_history(x: i32, y: i32, w: i32, h: i32, values: impl Iterator<Item = f64>, min: f64, max: f64) {
    let range = (max - min).max(1.0);

    draw::push_clip(x, y, w, h);
    draw::set_draw_color(Color::Light2);
    for i in 0..=4 {
        let gy = y + (h as f32 * (i as f32 / 4.0)) as i32;
        draw::draw_line(x, gy, x + w, gy);
    }

    let points: Vec<f64> = values.collect();
    if points.len() >= 2 {
        draw::set_draw_color(Color::from_rgb(0x33, 0x8c, 0xf2));
        draw::set_line_style(draw::LineStyle::Solid, 2);
        for i in 1..points.len() {
            let x0 = x + (w as f32 * ((i - 1) as f32 / (HISTORY_LEN.saturating_sub(1)) as f32)) as i32;
            let x1 = x + (w as f32 * (i as f32 / (HISTORY_LEN.saturating_sub(1)) as f32)) as i32;
            let norm0 = (((points[i - 1] - min) / range) as f32).clamp(0.0, 1.0);
            let norm1 = (((points[i] - min) / range) as f32).clamp(0.0, 1.0);
            let y0 = y + h - (norm0 * h as f32) as i32;
            let y1 = y + h - (norm1 * h as f32) as i32;
            draw::draw_line(x0, y0, x1, y1);
        }
        draw::set_line_style(draw::LineStyle::Solid, 0);
    }
    draw::pop_clip();
}
