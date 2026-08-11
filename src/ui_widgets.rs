//! Fonctions de dessin génériques (en-têtes de colonne, fond de ligne,
//! texte, graphes) et utilitaires de gestion des largeurs de colonnes,
//! réutilisées par les 4 tables (Détails/Services/Disques/Démarrage) et
//! les 2 graphes. Aucune dépendance à `Shared` ici -- ce sont des
//! fonctions purement "widget", sans logique métier.

use std::cell::RefCell;
use std::rc::Rc;

use fltk::{
    app,
    button::Button,
    draw,
    enums::{Align, Color, Font, FrameType},
    frame::Frame,
    group::Flex,
    input::Input,
    prelude::*,
    table::Table,
    window::Window,
};

use crate::state::HISTORY_LEN;
use crate::ui_style::*;

/// Dessine une cellule d'en-tête de colonne NATIVE (contexte ColHeader de
/// Fl_Table) : fond coloré façon bouton, titre, et flèche ▼/▲ optionnelle
/// si cette colonne est la colonne de tri actuellement active.
///
/// IMPORTANT -- pourquoi un en-tête natif plutôt que des Frame séparés :
/// le glisser-déposer pour redimensionner une colonne (col_resize) est
/// géré EXCLUSIVEMENT par l'en-tête natif de Fl_Table. On dessine donc
/// notre style DANS ce contexte natif plutôt qu'à côté, pour garder le
/// rendu voulu tout en récupérant le redimensionnement au glisser.
pub fn draw_col_header_cell(title: &str, sort_arrow: Option<&str>, x: i32, y: i32, w: i32, h: i32) {
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

/// Dessine le fond d'une cellule de contenu (zébré pair/impair, ou
/// surligné si `is_selected`) -- factorisé puisqu'utilisé par les 4
/// tables plutôt que dupliqué à chaque fois.
pub fn draw_row_background(x: i32, y: i32, w: i32, h: i32, row: i32, is_selected: bool) {
    draw::set_draw_color(if is_selected {
        Color::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2)
    } else if row % 2 == 0 {
        Color::from_rgb(ROW_BG_EVEN.0, ROW_BG_EVEN.1, ROW_BG_EVEN.2)
    } else {
        Color::from_rgb(ROW_BG_ODD.0, ROW_BG_ODD.1, ROW_BG_ODD.2)
    });
    draw::draw_rectf(x, y, w, h);
}

/// Dessine le texte d'une cellule de contenu, avec la couleur adaptée
/// selon l'état de sélection.
pub fn draw_row_text(text: &str, x: i32, y: i32, w: i32, h: i32, is_selected: bool) {
    draw::set_draw_color(if is_selected {
        Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2)
    } else {
        Color::from_rgb(TEXT_LIGHT.0, TEXT_LIGHT.1, TEXT_LIGHT.2)
    });
    draw::set_font(Font::Helvetica, 13);
    draw::draw_text2(text, x + 8, y, w - 8, h, Align::Left);
}

/// Répartit la largeur DISPONIBLE de `table` entre ses colonnes,
/// proportionnellement aux `weights` fournis. Utilisé à la construction
/// initiale et lors du tout premier redimensionnement détecté -- voir
/// `scale_col_widths` pour le réajustement des redimensionnements
/// suivants, qui préserve lui les largeurs choisies manuellement par
/// l'utilisateur (glisser-déposer sur une bordure de colonne).
pub fn apply_weighted_col_widths(table: &mut Table, weights: &[i32]) {
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

/// Réajuste les largeurs de colonnes existantes proportionnellement au
/// changement de largeur globale du tableau (ratio nouvelle/ancienne
/// largeur), plutôt que de les réinitialiser aux poids par défaut --
/// préserve un ajustement manuel de l'utilisateur lors d'un
/// redimensionnement de la fenêtre.
pub fn scale_col_widths(table: &mut Table, last_width: &mut i32) {
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

/// Regroupe une table et ses poids de colonnes, pour généraliser la
/// gestion des largeurs à un nombre arbitraire de tables (4 désormais :
/// Détails, Services, Disques, Démarrage) plutôt que de dupliquer la
/// logique une fois par table.
pub struct ManagedTable {
    pub table: Table,
    pub weights: &'static [i32],
}

/// Double filet de sécurité pour l'application initiale des largeurs de
/// colonnes : re-tente toutes les 100ms (jusqu'à 20 fois) tant que
/// `table.w()` ne semble pas encore refléter la taille réellement
/// affichée -- voir l'historique de ce problème de timing FLTK discuté
/// avec l'utilisateur (aucun point unique de synchronisation fiable
/// trouvé, d'où cette approche par sondage).
pub fn apply_all_initial_widths(tables: &mut Vec<ManagedTable>, retries_left: u32) {
    let ready = tables.iter().all(|t| t.table.w() > COL_WIDTH_MIN_SANE);
    if ready || retries_left == 0 {
        for t in tables.iter_mut() {
            apply_weighted_col_widths(&mut t.table, t.weights);
        }
        return;
    }
    let mut owned: Vec<ManagedTable> =
        tables.iter().map(|t| ManagedTable { table: t.table.clone(), weights: t.weights }).collect();
    app::add_timeout3(COL_WIDTH_RETRY_MS, move |_handle| {
        apply_all_initial_widths(&mut owned, retries_left - 1);
    });
}

/// Dessine une courbe simple à partir d'un historique de valeurs.
pub fn draw_history(x: i32, y: i32, w: i32, h: i32, values: impl Iterator<Item = f64>, min: f64, max: f64) {
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

/// Dessine l'historique ET des repères textuels directement sur le
/// graphe : valeur courante, pic observé, et bornes min/max de l'échelle.
pub fn draw_history_annotated(
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
        draw::draw_text2(&format!("{current:.1}{unit}"), x, y + 4, w - 8, 16, Align::Right | Align::Top);
    }

    if let Some(peak) = points.iter().copied().reduce(f64::max) {
        draw::set_draw_color(Color::from_rgb(TEXT_LIGHT.0, TEXT_LIGHT.1, TEXT_LIGHT.2));
        draw::draw_text2(&format!("pic {peak:.1}{unit}"), x, y + h - 20, w - 8, 16, Align::Right | Align::Bottom);
    }

    draw::pop_clip();
}

/// Affiche une petite fenêtre modale avec un ou plusieurs champs Input et
/// un bouton de validation ; retourne les valeurs saisies (dans l'ordre
/// des libellés fournis) si l'utilisateur valide, ou `None` s'il annule
/// ou ferme la fenêtre.
///
/// Implémentation "maison" plutôt qu'un éventuel `dialog::input()` de
/// FLTK : garantit un contrôle total et vérifié sur chaque widget utilisé
/// (Window/Flex/Input/Button), tous déjà éprouvés ailleurs dans ce
/// projet, plutôt que de risquer une signature d'API non vérifiée.
pub fn prompt_inputs(title: &str, fields: &[&str], confirm_label: &str) -> Option<Vec<String>> {
    let height = 70 + fields.len() as i32 * 34 + 50;
    let mut win = Window::default().with_size(440, height).with_label(title);
    win.set_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));

    let mut col = Flex::default_fill().column();
    col.set_margin(12);
    col.set_pad(8);

    let mut inputs: Vec<Input> = Vec::new();
    for field in fields {
        let mut row = Flex::default().row();
        let mut lbl = Frame::default().with_label(field);
        lbl.set_align(Align::Left | Align::Inside);
        lbl.set_label_color(Color::from_rgb(TEXT_LIGHT.0, TEXT_LIGHT.1, TEXT_LIGHT.2));
        row.fixed(&lbl, 150);
        let inp = Input::default();
        row.end();
        col.fixed(&row, 30);
        inputs.push(inp);
    }

    let mut btn_row = Flex::default().row();
    let mut btn_cancel = Button::default().with_label("Annuler");
    let mut btn_ok = Button::default().with_label(confirm_label);
    btn_ok.set_color(Color::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2));
    btn_ok.set_label_color(Color::from_rgb(TEXT_DARK.0, TEXT_DARK.1, TEXT_DARK.2));
    btn_row.end();
    col.fixed(&btn_row, 34);

    col.end();
    win.end();
    win.make_modal(true);
    win.show();

    let result: Rc<RefCell<Option<Vec<String>>>> = Rc::new(RefCell::new(None));

    {
        let result = result.clone();
        let inputs_clone = inputs.clone();
        let mut win_clone = win.clone();
        btn_ok.clone().set_callback(move |_| {
            let values: Vec<String> = inputs_clone.iter().map(|i| i.value()).collect();
            *result.borrow_mut() = Some(values);
            win_clone.hide();
        });
    }
    {
        let mut win_clone = win.clone();
        btn_cancel.set_callback(move |_| {
            win_clone.hide();
        });
    }

    while win.shown() {
        app::wait();
    }

    let taken = result.borrow_mut().take();
    taken
}
