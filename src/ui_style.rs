//! Constantes de style (couleurs, tailles, géométrie) et de comportement
//! partagées par tous les modules d'UI (ui_widgets, ui_tabs,
//! ui_callbacks, main). Isolées ici pour n'avoir qu'un seul endroit à
//! modifier pour ajuster l'apparence ou les dimensions -- avant ce
//! découpage, ces constantes étaient éparpillées en tête de main.rs.

pub const REFRESH_MS: i32 = 1500;
pub const ROW_HEIGHT: i32 = 26;
pub const COL_HEADER_HEIGHT: i32 = 28;

pub const ACCENT: (u8, u8, u8) = (0xbd, 0x93, 0xf9); // violet Dracula
pub const ACCENT_WARN: (u8, u8, u8) = (0xff, 0xb8, 0x6c); // orange Dracula
pub const ACCENT_CRIT: (u8, u8, u8) = (0xff, 0x55, 0x55); // rouge Dracula
pub const ACCENT_OK: (u8, u8, u8) = (0x50, 0xfa, 0x7b); // vert Dracula
pub const HEADER_BG: (u8, u8, u8) = (0x44, 0x47, 0x5a);
pub const ROW_BG_EVEN: (u8, u8, u8) = (0x28, 0x2a, 0x36);
pub const ROW_BG_ODD: (u8, u8, u8) = (0x21, 0x22, 0x2c);
pub const TEXT_LIGHT: (u8, u8, u8) = (0xf8, 0xf8, 0xf2);
pub const TEXT_DARK: (u8, u8, u8) = (0x1e, 0x1f, 0x29);

pub const COLUMN_TITLES: [&str; 5] = ["Nom", "PID", "CPU %", "Mémoire", "Statut"];
pub const DETAILS_COL_WEIGHTS: [i32; 5] = [4, 2, 2, 2, 2];

pub const SERVICE_COLUMN_TITLES: [&str; 3] = ["Nom", "État", "Description"];
pub const SERVICE_COL_WEIGHTS: [i32; 3] = [2, 2, 6];

pub const DISK_COLUMN_TITLES: [&str; 6] =
    ["Nom", "Point de montage", "Système de fichiers", "Utilisé", "Total", "Lecture / Écriture"];
pub const DISK_COL_WEIGHTS: [i32; 6] = [2, 3, 2, 2, 2, 3];

pub const STARTUP_COLUMN_TITLES: [&str; 4] = ["Nom", "Commande", "Source", "État"];
pub const STARTUP_COL_WEIGHTS: [i32; 4] = [2, 4, 2, 1];

// --- Géométrie explicite pour Tabs et ses pages ---
// (Fl_Tabs exige des Group comme enfants directs, pas des Flex -- d'où
// les coordonnées absolues plutôt qu'un layout Flex dynamique ici.)
pub const WINDOW_W: i32 = 1150;
pub const WINDOW_H: i32 = 700;
pub const TOP_BAR_H: i32 = 32;
pub const SEP_W: i32 = 2;
pub const RIGHT_PANEL_W: i32 = 340;
pub const TAB_BAR_H: i32 = 32;

pub const TABS_X: i32 = 0;
pub const TABS_Y: i32 = TOP_BAR_H;
pub const TABS_W: i32 = WINDOW_W - SEP_W - RIGHT_PANEL_W;
pub const TABS_H: i32 = WINDOW_H - TOP_BAR_H;

pub const PAGE_X: i32 = TABS_X;
pub const PAGE_Y: i32 = TABS_Y + TAB_BAR_H;
pub const PAGE_W: i32 = TABS_W;
pub const PAGE_H: i32 = TABS_H - TAB_BAR_H;

pub const COL_WIDTH_MIN_SANE: i32 = 150;
pub const COL_WIDTH_RETRY_MS: f64 = 0.1;
pub const COL_WIDTH_MAX_RETRIES: u32 = 20;
