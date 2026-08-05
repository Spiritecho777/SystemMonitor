mod process_monitor;
mod services;
mod state;
mod temperature;

use std::cell::RefCell;
use std::rc::Rc;

use fltk::{
    app,
    button::Button,
    draw,
    dialog,
    enums::{Align, Color, Event, Font, FrameType},
    frame::Frame,
    group::{Flex, Group, Tabs},
    input::Input,
    prelude::*,
    table::{Table, TableContext},
    window::Window,
};

use process_monitor::ProcessRow;
use services::{ServiceAction, ServiceRow};
use state::{human_bytes, AppState, HISTORY_LEN};

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

// Colonnes de l'onglet "Détails" (seul onglet de processus désormais --
// l'onglet "Processus" simplifié a été retiré : redondant avec Détails,
// qui reste la vue de référence).
const COLUMN_TITLES: [&str; 5] = ["Nom", "PID", "CPU %", "Mémoire", "Statut"];
const COLUMN_KEYS: [SortKey; 5] = [SortKey::Name, SortKey::Pid, SortKey::Cpu, SortKey::Memory, SortKey::Status];

const SERVICE_COLUMN_TITLES: [&str; 3] = ["Nom", "État", "Description"];

// --- Géométrie explicite pour Tabs et ses pages ---
//
// IMPORTANT : contrairement à Flex, `Fl_Tabs` ne recalcule PAS
// automatiquement la position/taille de ses enfants au moment de
// `.end()`, et surtout n'est PAS conçu pour accepter des `Flex` comme
// enfants directs -- Tabs attend des `Fl_Group` classiques (dont il lit
// le label pour construire sa barre d'onglets cliquable en haut). En
// utilisant Flex directement comme page d'onglet, Tabs ne les traite
// pas comme des pages exclusives : plusieurs onglets s'affichaient
// simultanément, côte à côte, chacun réduit à sa largeur minimale.
//
// La correction : Tabs contient des `Group` simples (label = titre de
// l'onglet), et c'est À L'INTÉRIEUR de chaque Group qu'on remet du Flex
// pour l'agencement interne (header + table) -- ça isole complètement
// les deux mécanismes de layout, chacun dans son cas d'usage prévu.
//
// Ces coordonnées sont des valeurs ABSOLUES explicites (pas de layout
// dynamique via Flex ici). Limite acceptée : au redimensionnement de la
// fenêtre, la zone des onglets ne se réajustera pas aussi finement que
// le reste de l'UI (piloté par Flex) -- compromis nécessaire pour que
// Tabs fonctionne correctement.
const WINDOW_W: i32 = 1150;
const WINDOW_H: i32 = 700;
const TOP_BAR_H: i32 = 28;
const SEP_W: i32 = 2;
const RIGHT_PANEL_W: i32 = 340;
const TAB_BAR_H: i32 = 30; // hauteur de la barre d'onglets cliquable, réservée par Fl_Tabs

const TABS_X: i32 = 0;
const TABS_Y: i32 = TOP_BAR_H;
const TABS_W: i32 = WINDOW_W - SEP_W - RIGHT_PANEL_W;
const TABS_H: i32 = WINDOW_H - TOP_BAR_H;

const PAGE_X: i32 = TABS_X;
const PAGE_Y: i32 = TABS_Y + TAB_BAR_H;
const PAGE_W: i32 = TABS_W;
const PAGE_H: i32 = TABS_H - TAB_BAR_H;

/// Tout ce qui doit survivre entre les callbacks (state applicatif + widgets
/// à rafraîchir + état d'UI transitoire comme le tri/le filtre/la sélection).
struct Shared {
    app_state: AppState,
    filtered_rows: Vec<ProcessRow>,
    sort_key: SortKey,
    sort_desc: bool,
    filter_text: String,
    selected_pid: Option<u32>,
    /// Filtre texte pour l'onglet Services (indépendant du filtre process).
    service_filter_text: String,
    filtered_services: Vec<ServiceRow>,
    selected_service: Option<String>,
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

    fn recompute_services(&mut self) {
        let needle = self.service_filter_text.to_lowercase();
        self.filtered_services = self
            .app_state
            .services()
            .iter()
            .filter(|s| {
                needle.is_empty()
                    || s.name.to_lowercase().contains(&needle)
                    || s.description.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect();
    }
}

/// Construit une ligne d'en-tête de colonnes stylée (UpBox + gras).
fn build_header_row(titles: &[&str]) -> (Flex, Vec<Frame>) {
    let header = Flex::default().row();
    let mut labels = Vec::new();
    for title in titles {
        let mut lbl = Frame::default().with_label(title);
        lbl.set_frame(FrameType::UpBox);
        lbl.set_label_font(Font::HelveticaBold);
        labels.push(lbl);
    }
    header.end();
    (header, labels)
}

/// Câble le clic sur un en-tête de colonne pour trier `shared`.
fn wire_sortable_header(header_labels: Vec<Frame>, keys: &'static [SortKey], shared: &Rc<RefCell<Shared>>, table: &Table) {
    for (i, mut lbl) in header_labels.into_iter().enumerate() {
        let shared = shared.clone();
        let mut table_clone = table.clone();
        lbl.handle(move |_widget, ev| {
            if ev == Event::Push {
                let mut sh = shared.borrow_mut();
                let key = keys[i];
                if sh.sort_key == key {
                    sh.sort_desc = !sh.sort_desc;
                } else {
                    sh.sort_key = key;
                    sh.sort_desc = true;
                }
                sh.recompute_rows();
                let n = sh.filtered_rows.len() as i32;
                drop(sh);
                table_clone.set_rows(n);
                table_clone.redraw();
                return true;
            }
            false
        });
    }
}

fn main() {
    let app = app::App::default();
    app::set_visible_focus(false);

    let mut window = Window::default().with_size(WINDOW_W, WINDOW_H).with_label("Gestionnaire de tâches");

    let mut root = Flex::default_fill().column();

    // --- barre du haut : CPU / RAM ---
    let mut top_bar = Flex::default().row();
    top_bar.set_frame(FrameType::FlatBox);
    let mut cpu_label = Frame::default().with_label("CPU : --");
    cpu_label.set_align(Align::Left | Align::Inside);
    let mut ram_label = Frame::default().with_label("RAM : --");
    ram_label.set_align(Align::Left | Align::Inside);
    top_bar.end();
    root.fixed(&top_bar, TOP_BAR_H);

    // --- corps : onglets (gauche) + panneau graphes (droite) ---
    let mut body = Flex::default().row();

    // ===== onglets : Détails / Services =====
    let mut tabs = Tabs::new(TABS_X, TABS_Y, TABS_W, TABS_H, None);

    // Variables déclarées ici (affectées dans les blocs ci-dessous) car
    // elles doivent survivre à la construction pour être câblées plus
    // bas (callbacks) et rafraîchies périodiquement (refresh_widgets).
    let mut filter_input: Input;
    let mut table: Table;
    let header_labels: Vec<Frame>;
    let mut table_services: Table;
    let mut service_filter_input: Input;
    let mut btn_start: Button;
    let mut btn_stop: Button;
    let mut btn_restart: Button;
    let mut service_status_label: Frame;

    // --- Onglet "Détails" (seul onglet de processus) ---
    {
        let mut grp_details = Group::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, "Détails");

        let mut tab_details = Flex::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, None).column();

        filter_input = Input::default();
        filter_input.set_tooltip("Filtrer par nom de process");
        tab_details.fixed(&filter_input, 28);

        let (header, labels) = build_header_row(&COLUMN_TITLES);
        tab_details.fixed(&header, 22);
        header_labels = labels;

        table = Table::default();
        table.set_rows(0);
        table.set_row_header(false);
        table.set_cols(COLUMN_TITLES.len() as i32);
        table.set_col_header(false); // en-tête géré à la main juste au-dessus (colonnes cliquables pour le tri)
        table.set_row_height_all(ROW_HEIGHT);
        table.set_col_resize(true);
        table.end();

        tab_details.end();
        grp_details.end();
    }

    // --- Onglet "Services" ---
    {
        let mut grp_services = Group::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, "Services");

        let mut tab_services = Flex::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, None).column();

        service_filter_input = Input::default();
        service_filter_input.set_tooltip("Filtrer par nom ou description de service");
        tab_services.fixed(&service_filter_input, 28);

        let (header, _labels) = build_header_row(&SERVICE_COLUMN_TITLES);
        tab_services.fixed(&header, 22);
        // Pas de tri cliquable sur cette table pour l'instant.

        table_services = Table::default();
        table_services.set_rows(0);
        table_services.set_row_header(false);
        table_services.set_cols(SERVICE_COLUMN_TITLES.len() as i32);
        table_services.set_col_header(false);
        table_services.set_row_height_all(ROW_HEIGHT);
        table_services.set_col_resize(true);
        table_services.end();

        // Barre de contrôle : Démarrer / Arrêter / Redémarrer.
        let mut control_bar = Flex::default().row();
        btn_start = Button::default().with_label("Démarrer");
        btn_stop = Button::default().with_label("Arrêter");
        btn_restart = Button::default().with_label("Redémarrer");
        control_bar.end();
        tab_services.fixed(&control_bar, 30);

        service_status_label = Frame::default().with_label("Sélectionne un service ci-dessus.");
        service_status_label.set_align(Align::Left | Align::Inside | Align::Wrap);
        tab_services.fixed(&service_status_label, 40);

        tab_services.end();
        grp_services.end();
    }

    tabs.end();

    // séparateur visuel
    let mut sep = Frame::default();
    sep.set_frame(FrameType::ThinDownFrame);
    body.fixed(&sep, SEP_W);

    // ===== droite : graphes =====
    let mut right = Flex::default().column();
    body.fixed(&right, RIGHT_PANEL_W);

    let mut temp_plot_heading = Frame::default().with_label("Historique température");
    temp_plot_heading.set_label_font(Font::HelveticaBold);
    temp_plot_heading.set_align(Align::Left | Align::Inside);
    right.fixed(&temp_plot_heading, 22);

    let mut temp_plot = Frame::default();
    temp_plot.set_frame(FrameType::DownBox);
    right.fixed(&temp_plot, 220);

    let mut cpu_plot_heading = Frame::default().with_label("Historique CPU");
    cpu_plot_heading.set_label_font(Font::HelveticaBold);
    cpu_plot_heading.set_align(Align::Left | Align::Inside);
    right.fixed(&cpu_plot_heading, 22);

    let mut cpu_plot = Frame::default();
    cpu_plot.set_frame(FrameType::DownBox);
    right.fixed(&cpu_plot, 200);

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
        service_filter_text: String::new(),
        filtered_services: Vec::new(),
        selected_service: None,
    }));
    {
        let mut sh = shared.borrow_mut();
        sh.recompute_rows();
        sh.recompute_services();
    }

    // --- dessin des cellules : table "Détails" ---
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

    // --- dessin des cellules : table "Services" ---
    {
        let shared = shared.clone();
        table_services.draw_cell(move |t, ctx, row, col, x, y, w, h| match ctx {
            TableContext::Cell => {
                let sh = shared.borrow();
                let is_selected = sh
                    .filtered_services
                    .get(row as usize)
                    .map(|s| Some(&s.name) == sh.selected_service.as_ref())
                    .unwrap_or(false);

                draw::push_clip(x, y, w, h);
                draw::set_draw_color(if is_selected { Color::Selection } else { Color::Background2 });
                draw::draw_rectf(x, y, w, h);

                if let Some(s) = sh.filtered_services.get(row as usize) {
                    let text = match col {
                        0 => s.name.clone(),
                        1 => s.state.clone(),
                        2 => s.description.clone(),
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

    // clic sur une ligne de process -> sélection (pour le bouton "Tuer")
    {
        let shared = shared.clone();
        let mut selection_label = selection_label.clone();
        let mut table_clone = table.clone();
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
                    drop(sh);
                    table_clone.redraw();
                    return true;
                }
            }
            false
        });
    }

    // clic sur une ligne de service -> sélection (pour Démarrer/Arrêter/Redémarrer)
    {
        let shared = shared.clone();
        let mut service_status_label = service_status_label.clone();
        let mut table_services_clone = table_services.clone();
        table_services.handle(move |t, ev| {
            if ev == Event::Push {
                let (row, _col) = (t.callback_row(), t.callback_col());
                if row >= 0 {
                    let mut sh = shared.borrow_mut();
                    if let Some(s) = sh.filtered_services.get(row as usize) {
                        let (name, state) = (s.name.clone(), s.state.clone());
                        sh.selected_service = Some(name.clone());
                        service_status_label.set_label(&format!("Sélection : {name} ({state})"));
                    }
                    drop(sh);
                    table_services_clone.redraw();
                    return true;
                }
            }
            false
        });
    }

    // clic sur un en-tête de colonne "Détails" -> tri
    wire_sortable_header(header_labels, &COLUMN_KEYS, &shared, &table);

    // filtre texte (onglet Détails)
    {
        let shared = shared.clone();
        let mut table_clone = table.clone();
        filter_input.set_trigger(fltk::enums::CallbackTrigger::Changed);
        filter_input.set_callback(move |inp| {
            let mut sh = shared.borrow_mut();
            sh.filter_text = inp.value();
            sh.recompute_rows();
            let n = sh.filtered_rows.len() as i32;
            drop(sh);
            table_clone.set_rows(n);
            table_clone.redraw();
        });
    }

    // filtre texte (onglet Services)
    {
        let shared = shared.clone();
        let mut table_services_clone = table_services.clone();
        service_filter_input.set_trigger(fltk::enums::CallbackTrigger::Changed);
        service_filter_input.set_callback(move |inp| {
            let mut sh = shared.borrow_mut();
            sh.service_filter_text = inp.value();
            sh.recompute_services();
            let n = sh.filtered_services.len() as i32;
            drop(sh);
            table_services_clone.set_rows(n);
            table_services_clone.redraw();
        });
    }

    // bouton "Tuer" (process)
    {
        let shared = shared.clone();
        kill_button.set_callback(move |_| {
            let mut sh = shared.borrow_mut();
            if let Some(pid) = sh.selected_pid.take() {
                sh.app_state.process_monitor.kill(pid);
            }
        });
    }

    // Boutons Démarrer / Arrêter / Redémarrer (services)
    //
    // IMPORTANT : ces actions nécessitent des privilèges élevés (root sur
    // Linux, administrateur sur Windows). Ce binaire ne demande aucune
    // élévation automatique -- si l'action échoue par manque de droits,
    // le message d'erreur système brut est affiché tel quel (boîte de
    // dialogue + label), pour que l'utilisateur comprenne la vraie cause
    // plutôt qu'un échec silencieux.
    for (btn, action) in [
        (&mut btn_start, ServiceAction::Start),
        (&mut btn_stop, ServiceAction::Stop),
        (&mut btn_restart, ServiceAction::Restart),
    ] {
        let shared = shared.clone();
        let mut service_status_label = service_status_label.clone();
        btn.set_callback(move |_| {
            let name = {
                let sh = shared.borrow();
                sh.selected_service.clone()
            };
            let Some(name) = name else {
                service_status_label.set_label("Sélectionne d'abord un service dans la liste.");
                return;
            };

            let result = {
                let sh = shared.borrow();
                sh.app_state.service_monitor.control(&name, action)
            };

            match result {
                Ok(()) => {
                    service_status_label.set_label(&format!("OK : action appliquée sur {name}."));
                    // Rafraîchissement immédiat pour refléter le nouvel état
                    // sans attendre le prochain cycle throttlé (voir
                    // state.rs::SERVICE_REFRESH_EVERY_N_TICKS).
                    let mut sh = shared.borrow_mut();
                    sh.app_state.service_monitor.refresh();
                    sh.recompute_services();
                }
                Err(e) => {
                    service_status_label.set_label(&format!("Échec sur {name} : {e}"));
                    dialog::alert_default(&format!("Échec de l'action sur le service \"{name}\" :\n\n{e}"));
                }
            }
        });
    }

    // --- graphes (dessin Cairo-like via le module `draw` de FLTK) ---
    {
        let shared = shared.clone();
        cpu_plot.draw(move |f| {
            let sh = shared.borrow();
            draw_history_annotated(
                f.x(),
                f.y(),
                f.w(),
                f.h(),
                sh.app_state.cpu_history.iter().copied(),
                0.0,
                100.0,
                "%",
            );
        });
    }
    {
        let shared = shared.clone();
        temp_plot.draw(move |f| {
            let sh = shared.borrow();
            let max_temp = sh.app_state.temp_history.iter().copied().fold(60.0_f64, f64::max);
            draw_history_annotated(
                f.x(),
                f.y(),
                f.w(),
                f.h(),
                sh.app_state.temp_history.iter().copied(),
                0.0,
                max_temp,
                "°C",
            );
        });
    }

    refresh_widgets(&shared, &mut cpu_label, &mut ram_label, &mut table, &mut table_services, &mut cpu_plot, &mut temp_plot);

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
                sh.recompute_services();
            }
            refresh_widgets(&shared, &mut cpu_label, &mut ram_label, &mut table, &mut table_services, &mut cpu_plot, &mut temp_plot);
        }
    }
}

fn refresh_widgets(
    shared: &Rc<RefCell<Shared>>,
    cpu_label: &mut Frame,
    ram_label: &mut Frame,
    table: &mut Table,
    table_services: &mut Table,
    cpu_plot: &mut Frame,
    temp_plot: &mut Frame,
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

    table_services.set_rows(sh.filtered_services.len() as i32);
    table_services.redraw();

    // Les closures .draw() de cpu_plot/temp_plot ne sont ré-invoquées par
    // FLTK que sur un événement de "damage" explicite -- jamais
    // automatiquement parce que cpu_history/temp_history ont changé en
    // arrière-plan.
    cpu_plot.redraw();
    temp_plot.redraw();

    drop(sh);
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

/// Dessine l'historique ET des repères textuels directement sur le
/// graphe : valeur courante, pic observé, et bornes min/max de l'échelle.
fn draw_history_annotated(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    values: impl Iterator<Item = f64> + Clone,
    min: f64,
    max: f64,
    unit: &str,
) {
    draw::push_clip(x, y, w, h);

    draw_history(x, y, w, h, values.clone(), min, max);

    let points: Vec<f64> = values.collect();

    draw::set_font(Font::HelveticaBold, 12);
    draw::set_draw_color(Color::Foreground);

    draw::draw_text2(&format!("{max:.0}{unit}"), x + 2, y + 2, w - 4, 14, Align::Left | Align::Top);
    draw::draw_text2(&format!("{min:.0}{unit}"), x + 2, y + h - 16, w - 4, 14, Align::Left | Align::Bottom);

    if let Some(&current) = points.last() {
        draw::draw_text2(
            &format!("{current:.1}{unit}"),
            x,
            y + 2,
            w - 4,
            16,
            Align::Right | Align::Top,
        );
    }

    if let Some(peak) = points.iter().copied().reduce(f64::max) {
        draw::draw_text2(
            &format!("pic {peak:.1}{unit}"),
            x,
            y + h - 18,
            w - 4,
            16,
            Align::Right | Align::Bottom,
        );
    }

    draw::pop_clip();
}
