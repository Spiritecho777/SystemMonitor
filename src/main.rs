#![cfg_attr(windows, windows_subsystem = "windows")]
mod process_monitor;
mod services;
mod state;
mod temperature;

use std::cell::RefCell;
use std::rc::Rc;

use fltk::{
    app,
    button::Button,
    dialog,
    draw,
    enums::{Align, Color, Event, Font, FrameType},
    frame::Frame,
    group::{Flex, Group, Tabs},
    image::PngImage,
    input::Input,
    prelude::*,
    table::{Table, TableContext},
    window::Window,
};
use fltk_theme::{color_themes, ColorTheme, SchemeType, WidgetScheme};

use process_monitor::ProcessRow;
use services::{ServiceAction, ServiceRow};
use state::{human_bytes, AppState, HISTORY_LEN};

const REFRESH_MS: i32 = 1500;
const ROW_HEIGHT: i32 = 26;
const COL_HEADER_HEIGHT: i32 = 28;

const ACCENT: (u8, u8, u8) = (0xbd, 0x93, 0xf9); // violet Dracula
const ACCENT_WARN: (u8, u8, u8) = (0xff, 0xb8, 0x6c); // orange Dracula
const ACCENT_CRIT: (u8, u8, u8) = (0xff, 0x55, 0x55); // rouge Dracula
const HEADER_BG: (u8, u8, u8) = (0x44, 0x47, 0x5a);
const ROW_BG_EVEN: (u8, u8, u8) = (0x28, 0x2a, 0x36);
const ROW_BG_ODD: (u8, u8, u8) = (0x21, 0x22, 0x2c);
const TEXT_LIGHT: (u8, u8, u8) = (0xf8, 0xf8, 0xf2);
const TEXT_DARK: (u8, u8, u8) = (0x1e, 0x1f, 0x29);

static ICON_BYTES: &[u8] = include_bytes!("../ressources/Icone.png");

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
const DETAILS_COL_WEIGHTS: [i32; 5] = [4, 2, 2, 2, 2];

const SERVICE_COLUMN_TITLES: [&str; 3] = ["Nom", "État", "Description"];
const SERVICE_COL_WEIGHTS: [i32; 3] = [2, 2, 6];

// --- Géométrie explicite pour Tabs et ses pages ---
const WINDOW_W: i32 = 1150;
const WINDOW_H: i32 = 700;
const TOP_BAR_H: i32 = 32;
const SEP_W: i32 = 2;
const RIGHT_PANEL_W: i32 = 340;
const TAB_BAR_H: i32 = 32;

const TABS_X: i32 = 0;
const TABS_Y: i32 = TOP_BAR_H;
const TABS_W: i32 = WINDOW_W - SEP_W - RIGHT_PANEL_W;
const TABS_H: i32 = WINDOW_H - TOP_BAR_H;

const PAGE_X: i32 = TABS_X;
const PAGE_Y: i32 = TABS_Y + TAB_BAR_H;
const PAGE_W: i32 = TABS_W;
const PAGE_H: i32 = TABS_H - TAB_BAR_H;

/// Tout ce qui doit survivre entre les callbacks.
struct Shared {
    app_state: AppState,
    filtered_rows: Vec<ProcessRow>,
    sort_key: SortKey,
    sort_desc: bool,
    filter_text: String,
    selected_pid: Option<u32>,
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

fn draw_col_header_cell(title: &str, sort_arrow: Option<&str>, x: i32, y: i32, w: i32, h: i32) {
    draw::push_clip(x, y, w, h);
    draw::draw_box(FrameType::UpBox, x, y, w, h, Color::from_rgb(HEADER_BG.0, HEADER_BG.1, HEADER_BG.2));
    draw::set_draw_color(Color::from_rgb(TEXT_LIGHT.0, TEXT_LIGHT.1, TEXT_LIGHT.2));
    draw::set_font(Font::HelveticaBold, 13);
    let label = match sort_arrow {
        Some(arrow) => format!("{title}{arrow}"),
        None => title.to_string(),
    };
    draw::draw_text2(&label, x + 6, y, w - 8, h, Align::Left);
    draw::pop_clip();
}

fn apply_weighted_col_widths(table: &mut Table, weights: &[i32]) {
    let total_weight: i32 = weights.iter().sum();
    if total_weight <= 0 {
        return;
    }
    let available = table.w();
    let n = weights.len();
    let mut used = 0;
    for (i, w) in weights.iter().enumerate() {
        let width = if i == n - 1 {
            (available - used).max(20)
        } else {
            ((available as i64 * *w as i64) / total_weight as i64).max(20) as i32
        };
        table.set_col_width(i as i32, width);
        used += width;
    }
    table.redraw();
}

fn scale_col_widths(table: &mut Table, last_width: &mut i32) {
    let new_w = table.w();
    if *last_width <= 0 || new_w <= 0 || new_w == *last_width {
        *last_width = new_w;
        return;
    }
    let ratio = new_w as f64 / *last_width as f64;
    let cols = table.cols();
    if cols <= 0 {
        *last_width = new_w;
        return;
    }
    let mut total = 0;
    for c in 0..cols {
        let w = table.col_width(c);
        let neww = ((w as f64) * ratio).round().max(20.0) as i32;
        table.set_col_width(c, neww);
        total += neww;
    }

    let last_col = cols - 1;
    let current_last = table.col_width(last_col);
    let diff = new_w - total;
    table.set_col_width(last_col, (current_last + diff).max(20));
    table.redraw();
    *last_width = new_w;
}

fn main() {
    let app = app::App::default();

    let theme = ColorTheme::new(color_themes::DARK_THEME);
    theme.apply();
    let scheme = WidgetScheme::new(SchemeType::Fluent);
    scheme.apply();

    app::set_visible_focus(false);
    app::set_font_size(13);

    let mut window = Window::default().with_size(WINDOW_W, WINDOW_H).with_label("Systeme Manager");
    window.set_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));

    let icon = PngImage::from_data(ICON_BYTES).unwrap();
    window.set_icon(Some(icon));

    let mut root = Flex::default_fill().column();
    root.set_margin(8);
    root.set_pad(8);

    // --- barre du haut : CPU / RAM ---
    let mut top_bar = Flex::default().row();
    top_bar.set_frame(FrameType::FlatBox);
    top_bar.set_color(Color::from_rgb(ROW_BG_EVEN.0, ROW_BG_EVEN.1, ROW_BG_EVEN.2));
    let mut cpu_label = Frame::default().with_label("CPU : --");
    cpu_label.set_align(Align::Left | Align::Inside);
    cpu_label.set_label_font(Font::HelveticaBold);
    cpu_label.set_label_size(14);
    let mut ram_label = Frame::default().with_label("RAM : --");
    ram_label.set_align(Align::Left | Align::Inside);
    ram_label.set_label_font(Font::HelveticaBold);
    ram_label.set_label_size(14);
    top_bar.end();
    root.fixed(&top_bar, TOP_BAR_H);

    // --- corps : onglets (gauche) + panneau graphes (droite) ---
    let mut body = Flex::default().row();
    body.set_pad(8);

    let mut tabs = Tabs::new(TABS_X, TABS_Y, TABS_W, TABS_H, None);
    tabs.set_frame(FrameType::FlatBox);

    let mut filter_input: Input;
    let mut table: Table;
    let mut table_services: Table;
    let mut service_filter_input: Input;
    let mut btn_start: Button;
    let mut btn_stop: Button;
    let mut btn_restart: Button;
    let mut service_status_label: Frame;

    // --- Onglet "Détails" ---
    {
        let mut grp_details = Group::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, "  Processus  ");

        let mut tab_details = Flex::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, None).column();
        tab_details.set_margin(6);
        tab_details.set_pad(6);

        filter_input = Input::default();
        filter_input.set_tooltip("Filtrer par nom de process");
        filter_input.set_text_size(13);
        tab_details.fixed(&filter_input, 30);

        table = Table::default();
        table.set_rows(0);
        table.set_row_header(false);
        table.set_cols(COLUMN_TITLES.len() as i32);
        table.set_col_header(true);
        table.set_col_header_height(COL_HEADER_HEIGHT);
        table.set_row_height_all(ROW_HEIGHT);
        table.set_col_resize(true);
        table.set_col_resize_min(40);
        table.end();

        tab_details.end();
        grp_details.end();

        apply_weighted_col_widths(&mut table, &DETAILS_COL_WEIGHTS);
    }

    // --- Onglet "Services" ---
    {
        let mut grp_services = Group::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, "  Services  ");

        let mut tab_services = Flex::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, None).column();
        tab_services.set_margin(6);
        tab_services.set_pad(6);

        service_filter_input = Input::default();
        service_filter_input.set_tooltip("Filtrer par nom ou description de service");
        service_filter_input.set_text_size(13);
        tab_services.fixed(&service_filter_input, 30);

        table_services = Table::default();
        table_services.set_rows(0);
        table_services.set_row_header(false);
        table_services.set_cols(SERVICE_COLUMN_TITLES.len() as i32);
        table_services.set_col_header(true);
        table_services.set_col_header_height(COL_HEADER_HEIGHT);
        table_services.set_row_height_all(ROW_HEIGHT);
        table_services.set_col_resize(true);
        table_services.set_col_resize_min(40);
        table_services.end();

        let mut control_bar = Flex::default().row();
        control_bar.set_pad(8);
        btn_start = Button::default().with_label("▶  Démarrer");
        btn_start.set_color(Color::from_rgb(0x50, 0xfa, 0x7b));
        btn_start.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
        btn_stop = Button::default().with_label("■  Arrêter");
        btn_stop.set_color(Color::from_rgb(ACCENT_CRIT.0, ACCENT_CRIT.1, ACCENT_CRIT.2));
        btn_stop.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
        btn_restart = Button::default().with_label("⟳  Redémarrer");
        btn_restart.set_color(Color::from_rgb(ACCENT_WARN.0, ACCENT_WARN.1, ACCENT_WARN.2));
        btn_restart.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
        control_bar.end();
        tab_services.fixed(&control_bar, 34);

        service_status_label = Frame::default().with_label("Sélectionne un service ci-dessus.");
        service_status_label.set_align(Align::Left | Align::Inside | Align::Wrap);
        service_status_label.set_label_size(12);
        tab_services.fixed(&service_status_label, 40);

        tab_services.end();
        grp_services.end();

        apply_weighted_col_widths(&mut table_services, &SERVICE_COL_WEIGHTS);
    }

    tabs.end();

    let mut sep = Frame::default();
    sep.set_frame(FrameType::FlatBox);
    sep.set_color(Color::from_rgb(HEADER_BG.0, HEADER_BG.1, HEADER_BG.2));
    body.fixed(&sep, SEP_W);

    // ===== droite : graphes =====
    let mut right = Flex::default().column();
    right.set_pad(6);
    body.fixed(&right, RIGHT_PANEL_W);

    let mut temp_plot_heading = Frame::default().with_label("Historique température");
    temp_plot_heading.set_label_font(Font::HelveticaBold);
    temp_plot_heading.set_label_size(14);
    temp_plot_heading.set_align(Align::Left | Align::Inside);
    right.fixed(&temp_plot_heading, 24);

    let mut temp_plot = Frame::default();
    temp_plot.set_frame(FrameType::RoundedBox);
    temp_plot.set_color(Color::from_rgb(ROW_BG_EVEN.0, ROW_BG_EVEN.1, ROW_BG_EVEN.2));
    right.fixed(&temp_plot, 220);

    let mut cpu_plot_heading = Frame::default().with_label("Historique CPU");
    cpu_plot_heading.set_label_font(Font::HelveticaBold);
    cpu_plot_heading.set_label_size(14);
    cpu_plot_heading.set_align(Align::Left | Align::Inside);
    right.fixed(&cpu_plot_heading, 24);

    let mut cpu_plot = Frame::default();
    cpu_plot.set_frame(FrameType::RoundedBox);
    cpu_plot.set_color(Color::from_rgb(ROW_BG_EVEN.0, ROW_BG_EVEN.1, ROW_BG_EVEN.2));
    right.fixed(&cpu_plot, 200);

    let mut kill_button = Button::default().with_label("✕  Tuer le process sélectionné");
    kill_button.set_color(Color::from_rgb(ACCENT_CRIT.0, ACCENT_CRIT.1, ACCENT_CRIT.2));
    kill_button.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
    right.fixed(&kill_button, 34);

    let mut selection_label = Frame::default().with_label("Aucune sélection");
    selection_label.set_align(Align::Left | Align::Inside);
    selection_label.set_label_size(12);
    right.fixed(&selection_label, 22);

    right.end();
    body.end();

    root.end();
    window.end();
    window.make_resizable(true);
    window.show();

    // --- Finalisation des largeurs de colonnes : double filet de sécurité ---
    const COL_WIDTH_MIN_SANE: i32 = 150;
    const COL_WIDTH_RETRY_MS: f64 = 0.1;
    const COL_WIDTH_MAX_RETRIES: u32 = 20;

    fn try_apply_initial_widths(table: &mut Table, table_services: &mut Table, retries_left: u32) {
        let ready = table.w() > COL_WIDTH_MIN_SANE && table_services.w() > COL_WIDTH_MIN_SANE;
        if ready {
            apply_weighted_col_widths(table, &DETAILS_COL_WEIGHTS);
            apply_weighted_col_widths(table_services, &SERVICE_COL_WEIGHTS);
            return;
        }
        if retries_left == 0 {
            apply_weighted_col_widths(table, &DETAILS_COL_WEIGHTS);
            apply_weighted_col_widths(table_services, &SERVICE_COL_WEIGHTS);
            return;
        }
        let mut table = table.clone();
        let mut table_services = table_services.clone();
        app::add_timeout3(COL_WIDTH_RETRY_MS, move |_handle| {
            try_apply_initial_widths(&mut table, &mut table_services, retries_left - 1);
        });
    }
    try_apply_initial_widths(&mut table.clone(), &mut table_services.clone(), COL_WIDTH_MAX_RETRIES);

    // --- Réajustement des colonnes au redimensionnement de la fenêtre ---
    {
        let mut table_for_resize = table.clone();
        let mut table_services_for_resize = table_services.clone();
        let mut last_details_w = table.w();
        let mut last_services_w = table_services.w();
        let mut first_resize_done = false;
        window.handle(move |_w, ev| {
            if ev == Event::Resize {
                if !first_resize_done {
                    apply_weighted_col_widths(&mut table_for_resize, &DETAILS_COL_WEIGHTS);
                    apply_weighted_col_widths(&mut table_services_for_resize, &SERVICE_COL_WEIGHTS);
                    first_resize_done = true;
                } else {
                    scale_col_widths(&mut table_for_resize, &mut last_details_w);
                    scale_col_widths(&mut table_services_for_resize, &mut last_services_w);
                }
                last_details_w = table_for_resize.w();
                last_services_w = table_services_for_resize.w();
            }
            false
        });
    }

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

    // --- dessin des cellules : table "Détails" (ColHeader + Cell) ---
    {
        let shared = shared.clone();
        table.draw_cell(move |t, ctx, row, col, x, y, w, h| match ctx {
            TableContext::ColHeader => {
                let sh = shared.borrow();
                let idx = col as usize;
                let arrow = if idx < COLUMN_KEYS.len() && COLUMN_KEYS[idx] == sh.sort_key {
                    Some(if sh.sort_desc { " ▼" } else { " ▲" })
                } else {
                    None
                };
                let title = COLUMN_TITLES.get(idx).copied().unwrap_or("");
                draw_col_header_cell(title, arrow, x, y, w, h);
                let _ = t;
            }
            TableContext::Cell => {
                let sh = shared.borrow();
                let is_selected = sh
                    .filtered_rows
                    .get(row as usize)
                    .map(|r| Some(r.pid) == sh.selected_pid)
                    .unwrap_or(false);

                draw::push_clip(x, y, w, h);
                draw::set_draw_color(if is_selected {
                    Color::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2)
                } else if row % 2 == 0 {
                    Color::from_rgb(ROW_BG_EVEN.0, ROW_BG_EVEN.1, ROW_BG_EVEN.2)
                } else {
                    Color::from_rgb(ROW_BG_ODD.0, ROW_BG_ODD.1, ROW_BG_ODD.2)
                });
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
                    draw::set_draw_color(if is_selected {
                        Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2)
                    } else {
                        Color::from_rgb(TEXT_LIGHT.0, TEXT_LIGHT.1, TEXT_LIGHT.2)
                    });
                    draw::set_font(Font::Helvetica, 13);
                    draw::draw_text2(&text, x + 8, y, w - 8, h, Align::Left);
                }

                draw::pop_clip();
                let _ = t;
            }
            _ => {}
        });
    }

    // --- dessin des cellules : table "Services" (ColHeader + Cell) ---
    {
        let shared = shared.clone();
        table_services.draw_cell(move |t, ctx, row, col, x, y, w, h| match ctx {
            TableContext::ColHeader => {
                let title = SERVICE_COLUMN_TITLES.get(col as usize).copied().unwrap_or("");
                draw_col_header_cell(title, None, x, y, w, h);
                let _ = t;
            }
            TableContext::Cell => {
                let sh = shared.borrow();
                let is_selected = sh
                    .filtered_services
                    .get(row as usize)
                    .map(|s| Some(&s.name) == sh.selected_service.as_ref())
                    .unwrap_or(false);

                draw::push_clip(x, y, w, h);
                draw::set_draw_color(if is_selected {
                    Color::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2)
                } else if row % 2 == 0 {
                    Color::from_rgb(ROW_BG_EVEN.0, ROW_BG_EVEN.1, ROW_BG_EVEN.2)
                } else {
                    Color::from_rgb(ROW_BG_ODD.0, ROW_BG_ODD.1, ROW_BG_ODD.2)
                });
                draw::draw_rectf(x, y, w, h);

                if let Some(s) = sh.filtered_services.get(row as usize) {
                    let text = match col {
                        0 => s.name.clone(),
                        1 => s.state.clone(),
                        2 => s.description.clone(),
                        _ => String::new(),
                    };
                    draw::set_draw_color(if is_selected {
                        Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2)
                    } else {
                        Color::from_rgb(TEXT_LIGHT.0, TEXT_LIGHT.1, TEXT_LIGHT.2)
                    });
                    draw::set_font(Font::Helvetica, 13);
                    draw::draw_text2(&text, x + 8, y, w - 8, h, Align::Left);
                }

                draw::pop_clip();
                let _ = t;
            }
            _ => {}
        });
    }

    {
        let shared = shared.clone();
        let mut selection_label = selection_label.clone();
        let mut table_clone = table.clone();
        table.handle(move |t, ev| {
            if ev != Event::Push {
                return false;
            }
            match t.callback_context() {
                TableContext::ColHeader => {
                    let col = t.callback_col();
                    if col < 0 || col as usize >= COLUMN_KEYS.len() {
                        return false;
                    }
                    let mut sh = shared.borrow_mut();
                    let key = COLUMN_KEYS[col as usize];
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
                    true
                }
                TableContext::Cell => {
                    let row = t.callback_row();
                    if row < 0 {
                        return false;
                    }
                    let mut sh = shared.borrow_mut();
                    if let Some(r) = sh.filtered_rows.get(row as usize) {
                        let (pid, name) = (r.pid, r.name.clone());
                        sh.selected_pid = Some(pid);
                        selection_label.set_label(&format!("Sélection : PID {} ({})", pid, name));
                    }
                    drop(sh);
                    table_clone.redraw();
                    true
                }
                _ => false,
            }
        });
    }

    {
        let shared = shared.clone();
        let mut service_status_label = service_status_label.clone();
        let mut table_services_clone = table_services.clone();
        table_services.handle(move |t, ev| {
            if ev == Event::Push && t.callback_context() == TableContext::Cell {
                let row = t.callback_row();
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

    // --- graphes ---
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

    cpu_plot.redraw();
    temp_plot.redraw();

    drop(sh);
}

fn draw_history(x: i32, y: i32, w: i32, h: i32, values: impl Iterator<Item = f64>, min: f64, max: f64) {
    let range = (max - min).max(1.0);

    draw::push_clip(x, y, w, h);
    draw::set_draw_color(Color::from_rgb(HEADER_BG.0, HEADER_BG.1, HEADER_BG.2));
    for i in 0..=4 {
        let gy = y + (h as f32 * (i as f32 / 4.0)) as i32;
        draw::draw_line(x, gy, x + w, gy);
    }

    let points: Vec<f64> = values.collect();
    if points.len() >= 2 {
        draw::set_draw_color(Color::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2));
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
    draw::set_draw_color(Color::from_rgb(TEXT_LIGHT.0, TEXT_LIGHT.1, TEXT_LIGHT.2));

    draw::draw_text2(&format!("{max:.0}{unit}"), x + 6, y + 4, w - 8, 14, Align::Left | Align::Top);
    draw::draw_text2(&format!("{min:.0}{unit}"), x + 6, y + h - 18, w - 8, 14, Align::Left | Align::Bottom);

    if let Some(&current) = points.last() {
        draw::set_draw_color(Color::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2));
        draw::draw_text2(
            &format!("{current:.1}{unit}"),
            x,
            y + 4,
            w - 8,
            16,
            Align::Right | Align::Top,
        );
    }

    if let Some(peak) = points.iter().copied().reduce(f64::max) {
        draw::set_draw_color(Color::from_rgb(TEXT_LIGHT.0, TEXT_LIGHT.1, TEXT_LIGHT.2));
        draw::draw_text2(
            &format!("pic {peak:.1}{unit}"),
            x,
            y + h - 20,
            w - 8,
            16,
            Align::Right | Align::Bottom,
        );
    }

    draw::pop_clip();
}
