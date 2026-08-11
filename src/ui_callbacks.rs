//! Câblage de tous les événements UI : dessin des cellules de table,
//! tri par clic sur en-tête, sélection de ligne, filtres texte, boutons
//! d'action (Tuer / Démarrer-Arrêter-Redémarrer / Exécuter une tâche /
//! Activer-Désactiver-Ajouter-Supprimer au démarrage), graphes, et
//! rafraîchissement périodique des widgets.
//!
//! Chaque fonction `wire_*` prend les widgets concernés par référence
//! mutable et `&SharedHandle`, exactement comme le faisait le code
//! inline de main.rs avant ce découpage -- aucune logique n'a changé,
//! seulement son emplacement.

use fltk::{
    button::Button,
    dialog, draw,
    enums::{CallbackTrigger, Event},
    frame::Frame,
    input::Input,
    prelude::*,
    table::{Table, TableContext},
    window::Window,
};

use crate::launcher;
use crate::services::ServiceAction;
use crate::state::{human_bytes, human_bytes_per_sec};
use crate::ui_shared::{SharedHandle, COLUMN_KEYS};
use crate::ui_style::*;
use crate::ui_widgets::{
    apply_all_initial_widths, apply_weighted_col_widths, draw_col_header_cell, draw_history_annotated,
    draw_row_background, draw_row_text, prompt_inputs, scale_col_widths, ManagedTable,
};

// =========================================================================
// Table "Détails"
// =========================================================================

pub fn wire_details_table(table: &mut Table, shared: &SharedHandle, selection_label: &Frame) {
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
                draw_row_background(x, y, w, h, row, is_selected);

                if let Some(r) = sh.filtered_rows.get(row as usize) {
                    let text = match col {
                        0 => r.name.clone(),
                        1 => r.pid.to_string(),
                        2 => format!("{:.1}", r.cpu_usage),
                        3 => human_bytes(r.memory_bytes),
                        4 => r.status.clone(),
                        _ => String::new(),
                    };
                    draw_row_text(&text, x, y, w, h, is_selected);
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
}

pub fn wire_details_filter(filter_input: &mut Input, table: &Table, shared: &SharedHandle) {
    let shared = shared.clone();
    let mut table_clone = table.clone();
    filter_input.set_trigger(CallbackTrigger::Changed);
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

// =========================================================================
// Table "Services"
// =========================================================================

pub fn wire_services_table(table: &mut Table, shared: &SharedHandle, status_label: &Frame) {
    {
        let shared = shared.clone();
        table.draw_cell(move |t, ctx, row, col, x, y, w, h| match ctx {
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
                draw_row_background(x, y, w, h, row, is_selected);

                if let Some(s) = sh.filtered_services.get(row as usize) {
                    let text = match col {
                        0 => s.name.clone(),
                        1 => s.state.clone(),
                        2 => s.description.clone(),
                        _ => String::new(),
                    };
                    draw_row_text(&text, x, y, w, h, is_selected);
                }

                draw::pop_clip();
                let _ = t;
            }
            _ => {}
        });
    }

    {
        let shared = shared.clone();
        let mut status_label = status_label.clone();
        let mut table_clone = table.clone();
        table.handle(move |t, ev| {
            if ev == Event::Push && t.callback_context() == TableContext::Cell {
                let row = t.callback_row();
                if row >= 0 {
                    let mut sh = shared.borrow_mut();
                    if let Some(s) = sh.filtered_services.get(row as usize) {
                        let (name, state) = (s.name.clone(), s.state.clone());
                        sh.selected_service = Some(name.clone());
                        status_label.set_label(&format!("Sélection : {name} ({state})"));
                    }
                    drop(sh);
                    table_clone.redraw();
                    return true;
                }
            }
            false
        });
    }
}

pub fn wire_services_filter(filter_input: &mut Input, table: &Table, shared: &SharedHandle) {
    let shared = shared.clone();
    let mut table_clone = table.clone();
    filter_input.set_trigger(CallbackTrigger::Changed);
    filter_input.set_callback(move |inp| {
        let mut sh = shared.borrow_mut();
        sh.service_filter_text = inp.value();
        sh.recompute_services();
        let n = sh.filtered_services.len() as i32;
        drop(sh);
        table_clone.set_rows(n);
        table_clone.redraw();
    });
}

pub fn wire_service_action_buttons(
    btn_start: &mut Button,
    btn_stop: &mut Button,
    btn_restart: &mut Button,
    shared: &SharedHandle,
    status_label: &Frame,
) {
    for (btn, action) in [
        (btn_start, ServiceAction::Start),
        (btn_stop, ServiceAction::Stop),
        (btn_restart, ServiceAction::Restart),
    ] {
        let shared = shared.clone();
        let mut status_label = status_label.clone();
        btn.set_callback(move |_| {
            let name = {
                let sh = shared.borrow();
                sh.selected_service.clone()
            };
            let Some(name) = name else {
                status_label.set_label("Sélectionne d'abord un service dans la liste.");
                return;
            };

            let result = {
                let sh = shared.borrow();
                sh.app_state.service_monitor.control(&name, action)
            };

            match result {
                Ok(()) => {
                    status_label.set_label(&format!("OK : action appliquée sur {name}."));
                    let mut sh = shared.borrow_mut();
                    sh.app_state.service_monitor.refresh();
                    sh.recompute_services();
                }
                Err(e) => {
                    status_label.set_label(&format!("Échec sur {name} : {e}"));
                    dialog::alert_default(&format!("Échec de l'action sur le service \"{name}\" :\n\n{e}"));
                }
            }
        });
    }
}

// =========================================================================
// Table "Disques" (lecture seule, pas de sélection ni de tri)
// =========================================================================

pub fn wire_disks_table(table: &mut Table, shared: &SharedHandle) {
    let shared = shared.clone();
    table.draw_cell(move |t, ctx, row, col, x, y, w, h| match ctx {
        TableContext::ColHeader => {
            let title = DISK_COLUMN_TITLES.get(col as usize).copied().unwrap_or("");
            draw_col_header_cell(title, None, x, y, w, h);
            let _ = t;
        }
        TableContext::Cell => {
            let sh = shared.borrow();
            let disk_rows = sh.app_state.disks();

            draw::push_clip(x, y, w, h);
            draw_row_background(x, y, w, h, row, false);

            if let Some(d) = disk_rows.get(row as usize) {
                let text: String = match col {
                    0 => d.name.clone(),
                    1 => d.mount_point.clone(),
                    2 => d.file_system.clone(),
                    3 => human_bytes(d.total_space.saturating_sub(d.available_space)),
                    4 => human_bytes(d.total_space),
                    5 => format!(
                        "▼ {}   ▲ {}",
                        human_bytes_per_sec(d.read_bytes_per_sec),
                        human_bytes_per_sec(d.write_bytes_per_sec)
                    ),
                    _ => String::new(),
                };
                draw_row_text(&text, x, y, w, h, false);
            }

            draw::pop_clip();
            let _ = t;
        }
        _ => {}
    });
}

// =========================================================================
// Table "Démarrage"
// =========================================================================

pub fn wire_startup_table(table: &mut Table, shared: &SharedHandle, status_label: &Frame) {
    {
        let shared = shared.clone();
        table.draw_cell(move |t, ctx, row, col, x, y, w, h| match ctx {
            TableContext::ColHeader => {
                let title = STARTUP_COLUMN_TITLES.get(col as usize).copied().unwrap_or("");
                draw_col_header_cell(title, None, x, y, w, h);
                let _ = t;
            }
            TableContext::Cell => {
                let sh = shared.borrow();
                let entries = sh.app_state.startup_entries();
                let is_selected = entries
                    .get(row as usize)
                    .map(|e| sh.selected_startup.as_ref() == Some(&(e.name.clone(), e.source)))
                    .unwrap_or(false);

                draw::push_clip(x, y, w, h);
                draw_row_background(x, y, w, h, row, is_selected);

                if let Some(e) = entries.get(row as usize) {
                    let text = match col {
                        0 => e.name.clone(),
                        1 => e.command.clone(),
                        2 => e.source.label().to_string(),
                        3 => if e.enabled { "Activé".to_string() } else { "Désactivé".to_string() },
                        _ => String::new(),
                    };
                    draw_row_text(&text, x, y, w, h, is_selected);
                }

                draw::pop_clip();
                let _ = t;
            }
            _ => {}
        });
    }

    {
        let shared = shared.clone();
        let mut status_label = status_label.clone();
        let mut table_clone = table.clone();
        table.handle(move |t, ev| {
            if ev == Event::Push && t.callback_context() == TableContext::Cell {
                let row = t.callback_row();
                if row >= 0 {
                    let mut sh = shared.borrow_mut();
                    let entry = sh.app_state.startup_entries().get(row as usize).cloned();
                    if let Some(e) = entry {
                        status_label.set_label(&format!(
                            "Sélection : {} ({}) -- {}",
                            e.name,
                            e.source.label(),
                            if e.enabled { "Activé" } else { "Désactivé" }
                        ));
                        sh.selected_startup = Some((e.name, e.source));
                    }
                    drop(sh);
                    table_clone.redraw();
                    return true;
                }
            }
            false
        });
    }
}

pub fn wire_startup_action_buttons(
    btn_enable: &mut Button,
    btn_disable: &mut Button,
    btn_add: &mut Button,
    btn_remove: &mut Button,
    shared: &SharedHandle,
    status_label: &Frame,
    table_startup: &mut Table,
) {
    for (btn, enable) in [(btn_enable, true), (btn_disable, false)] {
        let shared = shared.clone();
        let mut status_label = status_label.clone();
        let mut table_clone = table_startup.clone();
        btn.set_callback(move |_| {
            let selected = {
                let sh = shared.borrow();
                sh.selected_startup.clone()
            };
            let Some((name, source)) = selected else {
                status_label.set_label("Sélectionne d'abord une entrée dans la liste.");
                return;
            };

            let result = {
                let sh = shared.borrow();
                sh.app_state.startup_monitor.set_enabled(&name, source, enable)
            };

            match result {
                Ok(()) => {
                    let mut sh = shared.borrow_mut();
                    sh.app_state.startup_monitor.refresh();
                    let n = sh.app_state.startup_entries().len() as i32;
                    drop(sh);
                    status_label
                        .set_label(&format!("OK : {name} {}.", if enable { "activé" } else { "désactivé" }));
                    table_clone.set_rows(n);
                    table_clone.redraw();
                }
                Err(e) => {
                    status_label.set_label(&format!("Échec sur {name} : {e}"));
                    dialog::alert_default(&format!("Échec de l'action sur \"{name}\" :\n\n{e}"));
                }
            }
        });
    }

    {
        let shared = shared.clone();
        let mut status_label = status_label.clone();
        let mut table_clone = table_startup.clone();
        btn_add.set_callback(move |_| {
            let Some(values) =
                prompt_inputs("Ajouter au démarrage", &["Nom :", "Commande / chemin :"], "Ajouter")
            else {
                return;
            };
            let mut it = values.into_iter();
            let (Some(name), Some(command)) = (it.next(), it.next()) else { return };
            if name.trim().is_empty() || command.trim().is_empty() {
                status_label.set_label("Nom et commande sont obligatoires.");
                return;
            }

            let result = {
                let sh = shared.borrow();
                sh.app_state.startup_monitor.add_entry(&name, &command)
            };

            match result {
                Ok(()) => {
                    let mut sh = shared.borrow_mut();
                    sh.app_state.startup_monitor.refresh();
                    let n = sh.app_state.startup_entries().len() as i32;
                    drop(sh);
                    status_label.set_label(&format!("OK : \"{name}\" ajouté au démarrage."));
                    table_clone.set_rows(n);
                    table_clone.redraw();
                }
                Err(e) => {
                    status_label.set_label(&format!("Échec de l'ajout : {e}"));
                    dialog::alert_default(&format!("Échec de l'ajout de \"{name}\" :\n\n{e}"));
                }
            }
        });
    }

    {
        let shared = shared.clone();
        let mut status_label = status_label.clone();
        let mut table_clone = table_startup.clone();
        btn_remove.set_callback(move |_| {
            let selected = {
                let sh = shared.borrow();
                sh.selected_startup.clone()
            };
            let Some((name, source)) = selected else {
                status_label.set_label("Sélectionne d'abord une entrée dans la liste.");
                return;
            };

            let result = {
                let sh = shared.borrow();
                sh.app_state.startup_monitor.remove_entry(&name, source)
            };

            match result {
                Ok(()) => {
                    let mut sh = shared.borrow_mut();
                    sh.app_state.startup_monitor.refresh();
                    sh.selected_startup = None;
                    let n = sh.app_state.startup_entries().len() as i32;
                    drop(sh);
                    status_label.set_label(&format!("OK : \"{name}\" supprimé."));
                    table_clone.set_rows(n);
                    table_clone.redraw();
                }
                Err(e) => {
                    status_label.set_label(&format!("Échec de la suppression : {e}"));
                    dialog::alert_default(&format!("Échec de la suppression de \"{name}\" :\n\n{e}"));
                }
            }
        });
    }
}

// =========================================================================
// Boutons "globaux" (barre du haut / panneau droit)
// =========================================================================

pub fn wire_kill_button(kill_button: &mut Button, shared: &SharedHandle) {
    let shared = shared.clone();
    kill_button.set_callback(move |_| {
        let mut sh = shared.borrow_mut();
        if let Some(pid) = sh.selected_pid.take() {
            sh.app_state.process_monitor.kill(pid);
        }
    });
}

pub fn wire_run_task_button(run_task_btn: &mut Button) {
    run_task_btn.set_callback(move |_| {
        if let Some(values) = prompt_inputs("Exécuter une tâche", &["Commande / chemin :"], "Exécuter") {
            if let Some(cmd) = values.into_iter().next() {
                if let Err(e) = launcher::run_task(&cmd) {
                    dialog::alert_default(&format!("Erreur : {e}"));
                }
            }
        }
    });
}

// =========================================================================
// Graphes
// =========================================================================

pub fn wire_graphs(cpu_plot: &mut Frame, temp_plot: &mut Frame, shared: &SharedHandle) {
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
}

// =========================================================================
// Gestion des largeurs de colonnes : initiale + au redimensionnement
// =========================================================================

/// Regroupe en un seul appel les deux mécanismes de gestion des largeurs
/// de colonnes (application initiale + réajustement au redimensionnement
/// de la fenêtre), pour les 4 tables à la fois plutôt que de dupliquer le
/// câblage une fois par table.
pub fn setup_column_width_management(window: &mut Window, tables: Vec<(Table, &'static [i32])>) {
    // --- Application initiale (avec ré-essais, voir apply_all_initial_widths) ---
    {
        let mut managed: Vec<ManagedTable> =
            tables.iter().map(|(t, w)| ManagedTable { table: t.clone(), weights: w }).collect();
        apply_all_initial_widths(&mut managed, COL_WIDTH_MAX_RETRIES);
    }

    // --- Réajustement au redimensionnement de la fenêtre ---
    let mut managed: Vec<ManagedTable> =
        tables.into_iter().map(|(t, w)| ManagedTable { table: t, weights: w }).collect();
    let mut last_widths: Vec<i32> = managed.iter().map(|m| m.table.w()).collect();
    let mut first_resize_done = false;
    window.handle(move |_w, ev| {
        if ev == Event::Resize {
            if !first_resize_done {
                for m in managed.iter_mut() {
                    apply_weighted_col_widths(&mut m.table, m.weights);
                }
                first_resize_done = true;
            } else {
                for (m, last_w) in managed.iter_mut().zip(last_widths.iter_mut()) {
                    scale_col_widths(&mut m.table, last_w);
                }
            }
            for (m, last_w) in managed.iter_mut().zip(last_widths.iter_mut()) {
                *last_w = m.table.w();
            }
        }
        false
    });
}

// =========================================================================
// Rafraîchissement périodique
// =========================================================================

pub fn refresh_widgets(
    shared: &SharedHandle,
    cpu_label: &mut Frame,
    ram_label: &mut Frame,
    table: &mut Table,
    table_services: &mut Table,
    table_disks: &mut Table,
    table_startup: &mut Table,
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

    table_disks.set_rows(sh.app_state.disks().len() as i32);
    table_disks.redraw();

    table_startup.set_rows(sh.app_state.startup_entries().len() as i32);
    table_startup.redraw();

    cpu_plot.redraw();
    temp_plot.redraw();

    drop(sh);
}
