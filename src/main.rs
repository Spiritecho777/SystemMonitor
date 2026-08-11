#![cfg_attr(windows, windows_subsystem = "windows")]
mod disks;
mod launcher;
mod process_monitor;
mod services;
mod startup;
mod state;
mod temperature;
mod ui_callbacks;
mod ui_shared;
mod ui_style;
mod ui_tabs;
mod ui_widgets;

use std::cell::RefCell;
use std::rc::Rc;

use fltk::{
    app,
    button::Button,
    enums::{Align, Color, Font, FrameType},
    frame::Frame,
    group::{Flex, Tabs},
    image::PngImage,
    prelude::*,
    window::Window,
};
use fltk_theme::{color_themes, ColorTheme, SchemeType, WidgetScheme};

use ui_callbacks::{
    refresh_widgets, setup_column_width_management, wire_details_filter, wire_details_table,
    wire_disks_table, wire_graphs, wire_kill_button, wire_run_task_button, wire_service_action_buttons,
    wire_services_filter, wire_services_table, wire_startup_action_buttons, wire_startup_table,
};
use ui_shared::{Shared, SharedHandle};
use ui_style::*;
use ui_tabs::{build_details_tab, build_disks_tab, build_services_tab, build_startup_tab};

static ICON_BYTES: &[u8] = include_bytes!("../ressources/Icone.png");

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

    // --- barre du haut : CPU / RAM / Exécuter une tâche ---
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
    let mut run_task_btn = Button::default().with_label("▶  Exécuter une tâche");
    run_task_btn.set_color(Color::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2));
    run_task_btn.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
    top_bar.fixed(&run_task_btn, 220);
    top_bar.end();
    root.fixed(&top_bar, TOP_BAR_H);

    // --- corps : onglets (gauche) + panneau graphes (droite) ---
    let mut body = Flex::default().row();
    body.set_pad(8);

    let mut tabs = Tabs::new(TABS_X, TABS_Y, TABS_W, TABS_H, None);
    tabs.set_frame(FrameType::FlatBox);

    let mut details_tab = build_details_tab();
    let mut services_tab = build_services_tab();
    let mut disks_tab = build_disks_tab();
    let mut startup_tab = build_startup_tab();

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

    // --- Gestion des largeurs de colonnes (initiale + au redimensionnement) ---
    // Regroupée en un seul appel pour les 4 tables -- voir
    // ui_callbacks::setup_column_width_management.
    setup_column_width_management(
        &mut window,
        vec![
            (details_tab.table.clone(), &DETAILS_COL_WEIGHTS[..]),
            (services_tab.table.clone(), &SERVICE_COL_WEIGHTS[..]),
            (disks_tab.table.clone(), &DISK_COL_WEIGHTS[..]),
            (startup_tab.table.clone(), &STARTUP_COL_WEIGHTS[..]),
        ],
    );

    // --- état partagé ---
    let shared: SharedHandle = Rc::new(RefCell::new(Shared::new()));
    {
        let mut sh = shared.borrow_mut();
        sh.recompute_rows();
        sh.recompute_services();
    }

    // --- câblage des tables, filtres et boutons ---
    wire_details_table(&mut details_tab.table, &shared, &selection_label);
    wire_details_filter(&mut details_tab.filter_input, &details_tab.table, &shared);

    wire_services_table(&mut services_tab.table, &shared, &services_tab.status_label);
    wire_services_filter(&mut services_tab.filter_input, &services_tab.table, &shared);
    wire_service_action_buttons(
        &mut services_tab.btn_start,
        &mut services_tab.btn_stop,
        &mut services_tab.btn_restart,
        &shared,
        &services_tab.status_label,
    );

    wire_disks_table(&mut disks_tab.table, &shared);

    wire_startup_table(&mut startup_tab.table, &shared, &startup_tab.status_label);
    wire_startup_action_buttons(
        &mut startup_tab.btn_enable,
        &mut startup_tab.btn_disable,
        &mut startup_tab.btn_add,
        &mut startup_tab.btn_remove,
        &shared,
        &startup_tab.status_label,
        &mut startup_tab.table,
    );

    wire_kill_button(&mut kill_button, &shared);
    wire_run_task_button(&mut run_task_btn);

    wire_graphs(&mut cpu_plot, &mut temp_plot, &shared);

    refresh_widgets(
        &shared,
        &mut cpu_label,
        &mut ram_label,
        &mut details_tab.table,
        &mut services_tab.table,
        &mut disks_tab.table,
        &mut startup_tab.table,
        &mut cpu_plot,
        &mut temp_plot,
    );

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
            refresh_widgets(
                &shared,
                &mut cpu_label,
                &mut ram_label,
                &mut details_tab.table,
                &mut services_tab.table,
                &mut disks_tab.table,
                &mut startup_tab.table,
                &mut cpu_plot,
                &mut temp_plot,
            );
        }
    }
}
