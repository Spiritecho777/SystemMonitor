//! Construction de chaque onglet (Détails / Services / Disques /
//! Démarrage). Chaque fonction `build_*_tab()` construit son
//! `Group`+`Flex` et retourne une petite struct qui regroupe les widgets
//! créés, pour que main.rs puisse ensuite les câbler (voir
//! ui_callbacks.rs) sans avoir à connaître les détails de construction.

use fltk::{
    button::Button,
    enums::{Align, Color, FrameType},
    frame::Frame,
    group::{Flex, Group},
    input::Input,
    prelude::*,
    table::Table,
};

use crate::ui_style::*;

pub struct DetailsTab {
    pub filter_input: Input,
    pub table: Table,
}

pub fn build_details_tab() -> DetailsTab {
    let mut grp = Group::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, "  Processus  ");

    let mut flex = Flex::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, None).column();
    flex.set_margin(6);
    flex.set_pad(6);

    let mut filter_input = Input::default();
    filter_input.set_tooltip("Filtrer par nom de process");
    filter_input.set_text_size(13);
    flex.fixed(&filter_input, 30);

    let mut table = Table::default();
    table.set_rows(0);
    table.set_row_header(false);
    table.set_cols(COLUMN_TITLES.len() as i32);
    // En-tête NATIF (pas de Frame séparés) : condition nécessaire pour
    // que le glisser-déposer de redimensionnement de colonne fonctionne.
    table.set_col_header(true);
    table.set_col_header_height(COL_HEADER_HEIGHT);
    table.set_row_height_all(ROW_HEIGHT);
    table.set_col_resize(true);
    table.set_col_resize_min(40);
    table.end();

    flex.end();
    grp.end();

    DetailsTab { filter_input, table }
}

pub struct ServicesTab {
    pub filter_input: Input,
    pub table: Table,
    pub btn_start: Button,
    pub btn_stop: Button,
    pub btn_restart: Button,
    pub status_label: Frame,
}

pub fn build_services_tab() -> ServicesTab {
    let mut grp = Group::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, "  Services  ");

    let mut flex = Flex::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, None).column();
    flex.set_margin(6);
    flex.set_pad(6);

    let mut filter_input = Input::default();
    filter_input.set_tooltip("Filtrer par nom ou description de service");
    filter_input.set_text_size(13);
    flex.fixed(&filter_input, 30);

    let mut table = Table::default();
    table.set_rows(0);
    table.set_row_header(false);
    table.set_cols(SERVICE_COLUMN_TITLES.len() as i32);
    table.set_col_header(true);
    table.set_col_header_height(COL_HEADER_HEIGHT);
    table.set_row_height_all(ROW_HEIGHT);
    table.set_col_resize(true);
    table.set_col_resize_min(40);
    table.end();

    let mut control_bar = Flex::default().row();
    control_bar.set_pad(8);
    let mut btn_start = Button::default().with_label("▶  Démarrer");
    btn_start.set_color(Color::from_rgb(ACCENT_OK.0, ACCENT_OK.1, ACCENT_OK.2));
    btn_start.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
    let mut btn_stop = Button::default().with_label("■  Arrêter");
    btn_stop.set_color(Color::from_rgb(ACCENT_CRIT.0, ACCENT_CRIT.1, ACCENT_CRIT.2));
    btn_stop.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
    let mut btn_restart = Button::default().with_label("⟳  Redémarrer");
    btn_restart.set_color(Color::from_rgb(ACCENT_WARN.0, ACCENT_WARN.1, ACCENT_WARN.2));
    btn_restart.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
    control_bar.end();
    flex.fixed(&control_bar, 34);

    let mut status_label = Frame::default().with_label("Sélectionne un service ci-dessus.");
    status_label.set_align(Align::Left | Align::Inside | Align::Wrap);
    status_label.set_label_size(12);
    flex.fixed(&status_label, 40);

    flex.end();
    grp.end();

    ServicesTab { filter_input, table, btn_start, btn_stop, btn_restart, status_label }
}

pub struct DisksTab {
    pub table: Table,
}

pub fn build_disks_tab() -> DisksTab {
    let mut grp = Group::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, "  Disques  ");

    let mut flex = Flex::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, None).column();
    flex.set_margin(6);
    flex.set_pad(6);

    let mut table = Table::default();
    table.set_rows(0);
    table.set_row_header(false);
    table.set_cols(DISK_COLUMN_TITLES.len() as i32);
    table.set_col_header(true);
    table.set_col_header_height(COL_HEADER_HEIGHT);
    table.set_row_height_all(ROW_HEIGHT);
    table.set_col_resize(true);
    table.set_col_resize_min(40);
    table.end();

    flex.end();
    grp.end();

    DisksTab { table }
}

pub struct StartupTab {
    pub table: Table,
    pub btn_enable: Button,
    pub btn_disable: Button,
    pub btn_add: Button,
    pub btn_remove: Button,
    pub status_label: Frame,
}

pub fn build_startup_tab() -> StartupTab {
    let mut grp = Group::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, "  Démarrage  ");

    let mut flex = Flex::new(PAGE_X, PAGE_Y, PAGE_W, PAGE_H, None).column();
    flex.set_margin(6);
    flex.set_pad(6);

    let mut table = Table::default();
    table.set_rows(0);
    table.set_row_header(false);
    table.set_cols(STARTUP_COLUMN_TITLES.len() as i32);
    table.set_col_header(true);
    table.set_col_header_height(COL_HEADER_HEIGHT);
    table.set_row_height_all(ROW_HEIGHT);
    table.set_col_resize(true);
    table.set_col_resize_min(40);
    table.end();

    let mut control_bar = Flex::default().row();
    control_bar.set_pad(8);
    let mut btn_enable = Button::default().with_label("▶  Activer");
    btn_enable.set_color(Color::from_rgb(ACCENT_OK.0, ACCENT_OK.1, ACCENT_OK.2));
    btn_enable.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
    let mut btn_disable = Button::default().with_label("■  Désactiver");
    btn_disable.set_color(Color::from_rgb(ACCENT_CRIT.0, ACCENT_CRIT.1, ACCENT_CRIT.2));
    btn_disable.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
    let mut btn_add = Button::default().with_label("+  Ajouter");
    btn_add.set_color(Color::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2));
    btn_add.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
    let mut btn_remove = Button::default().with_label("✕  Supprimer");
    btn_remove.set_color(Color::from_rgb(ACCENT_WARN.0, ACCENT_WARN.1, ACCENT_WARN.2));
    btn_remove.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
    control_bar.end();
    flex.fixed(&control_bar, 34);

    let mut status_label = Frame::default().with_label("Sélectionne une entrée ci-dessus.");
    status_label.set_align(Align::Left | Align::Inside | Align::Wrap);
    status_label.set_label_size(12);
    flex.fixed(&status_label, 40);

    flex.end();
    grp.end();

    StartupTab { table, btn_enable, btn_disable, btn_add, btn_remove, status_label }
}
