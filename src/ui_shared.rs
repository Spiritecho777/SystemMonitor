//! État partagé entre tous les callbacks de l'UI (tri, filtre,
//! sélection) + état applicatif (`AppState`). Regroupé dans une unique
//! struct `Shared`, elle-même enveloppée dans `SharedHandle`
//! (`Rc<RefCell<Shared>>`) pour être clonée/partagée entre toutes les
//! closures de callback -- exactement le même mécanisme qu'avant ce
//! découpage, simplement déplacé hors de main.rs pour que
//! ui_callbacks.rs et ui_tabs.rs puissent y accéder aussi.

use std::cell::RefCell;
use std::rc::Rc;

use crate::process_monitor::ProcessRow;
use crate::services::ServiceRow;
use crate::startup::StartupSource;
use crate::state::AppState;

#[derive(Clone, Copy, PartialEq)]
pub enum SortKey {
    Name,
    Pid,
    Cpu,
    Memory,
    Status,
}

pub const COLUMN_KEYS: [SortKey; 5] =
    [SortKey::Name, SortKey::Pid, SortKey::Cpu, SortKey::Memory, SortKey::Status];

/// Tout ce qui doit survivre entre les callbacks.
pub struct Shared {
    pub app_state: AppState,
    pub filtered_rows: Vec<ProcessRow>,
    pub sort_key: SortKey,
    pub sort_desc: bool,
    pub filter_text: String,
    pub selected_pid: Option<u32>,
    pub service_filter_text: String,
    pub filtered_services: Vec<ServiceRow>,
    pub selected_service: Option<String>,
    pub selected_startup: Option<(String, StartupSource)>,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            app_state: AppState::new(),
            filtered_rows: Vec::new(),
            sort_key: SortKey::Cpu,
            sort_desc: true,
            filter_text: String::new(),
            selected_pid: None,
            service_filter_text: String::new(),
            filtered_services: Vec::new(),
            selected_service: None,
            selected_startup: None,
        }
    }

    pub fn recompute_rows(&mut self) {
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

    pub fn recompute_services(&mut self) {
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

/// Alias pratique -- évite de répéter `Rc<RefCell<Shared>>` dans toutes
/// les signatures de fonctions de ui_tabs.rs / ui_callbacks.rs / main.rs.
pub type SharedHandle = Rc<RefCell<Shared>>;
