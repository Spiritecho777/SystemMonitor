/// D'où vient cette entrée de démarrage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupSource {
    /// Linux : ~/.config/autostart (utilisateur, modifiable sans droits
    /// particuliers).
    UserAutostart,
    /// Linux : /etc/xdg/autostart (système, en pratique lecture seule --
    /// désactiver une de ces entrées crée une COPIE utilisateur qui la
    /// masque, conformément au mécanisme de surcharge standard de la
    /// spec XDG -- voir le commentaire détaillé dans set_enabled_impl).
    SystemAutostart,
    /// Windows : HKCU\...\Run (utilisateur, pas de droits admin requis).
    UserRegistry,
    /// Windows : HKLM\...\Run (machine entière, droits administrateur
    /// requis pour modifier -- ce binaire ne tente AUCUNE élévation
    /// automatique, cohérent avec le choix déjà fait pour les services).
    MachineRegistry,
}

impl StartupSource {
    pub fn label(&self) -> &'static str {
        match self {
            StartupSource::UserAutostart => "Autostart (utilisateur)",
            StartupSource::SystemAutostart => "Autostart (système)",
            StartupSource::UserRegistry => "Registre (HKCU)",
            StartupSource::MachineRegistry => "Registre (HKLM)",
        }
    }
}

#[derive(Clone, Debug)]
pub struct StartupEntry {
    pub name: String,
    pub command: String,
    pub source: StartupSource,
    pub enabled: bool,
}

/// IMPORTANT -- portée volontairement limitée : ce module permet de
/// lister, activer/désactiver et ajouter/supprimer des entrées de
/// démarrage standard (XDG autostart sur Linux, clé Run du registre sur
/// Windows). Il ne gère PAS les raccourcis du dossier "Démarrage" de
/// Windows (shell:startup) ni les tâches planifiées -- ce sont des
/// mécanismes distincts, hors scope de cette première itération.
pub struct StartupMonitor {
    entries: Vec<StartupEntry>,
}

impl StartupMonitor {
    pub fn new() -> Self {
        let mut s = Self { entries: Vec::new() };
        s.refresh();
        s
    }

    pub fn refresh(&mut self) {
        self.entries = fetch_entries();
    }

    pub fn entries(&self) -> &[StartupEntry] {
        &self.entries
    }

    /// Active/désactive une entrée existante. Retourne `Err` si
    /// l'opération nécessite des droits que ce process n'a pas (ex:
    /// modifier HKLM sans admin, ou modifier un fichier système sans
    /// droits root), ou en cas d'erreur d'E/S.
    pub fn set_enabled(&self, name: &str, source: StartupSource, enabled: bool) -> Result<(), String> {
        set_enabled_impl(name, source, enabled)
    }

    /// Ajoute une nouvelle entrée de démarrage pointant vers `command`.
    /// Toujours ajoutée dans le scope UTILISATEUR (UserAutostart /
    /// UserRegistry) -- jamais dans le scope machine, qui nécessiterait
    /// des droits admin.
    pub fn add_entry(&self, name: &str, command: &str) -> Result<(), String> {
        add_entry_impl(name, command)
    }

    pub fn remove_entry(&self, name: &str, source: StartupSource) -> Result<(), String> {
        remove_entry_impl(name, source)
    }
}

fn fetch_entries() -> Vec<StartupEntry> {
    #[cfg(target_os = "linux")]
    {
        linux_impl::fetch_entries()
    }
    #[cfg(target_os = "windows")]
    {
        windows_impl::fetch_entries()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

fn set_enabled_impl(name: &str, source: StartupSource, enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        linux_impl::set_enabled_impl(name, source, enabled)
    }
    #[cfg(target_os = "windows")]
    {
        windows_impl::set_enabled_impl(name, source, enabled)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (name, source, enabled);
        Err("Non supporté sur cette plateforme.".to_string())
    }
}

fn add_entry_impl(name: &str, command: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        linux_impl::add_entry_impl(name, command)
    }
    #[cfg(target_os = "windows")]
    {
        windows_impl::add_entry_impl(name, command)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (name, command);
        Err("Non supporté sur cette plateforme.".to_string())
    }
}

fn remove_entry_impl(name: &str, source: StartupSource) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        linux_impl::remove_entry_impl(name, source)
    }
    #[cfg(target_os = "windows")]
    {
        windows_impl::remove_entry_impl(name, source)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (name, source);
        Err("Non supporté sur cette plateforme.".to_string())
    }
}

// =========================================================================
// Implémentation Linux : fichiers .desktop XDG autostart
// =========================================================================
#[cfg(target_os = "linux")]
mod linux_impl {
    use super::{StartupEntry, StartupSource};
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn user_autostart_dir() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            Path::new(&home).join(".config")
        });
        base.join("autostart")
    }

    fn system_autostart_dirs() -> Vec<PathBuf> {
        std::env::var("XDG_CONFIG_DIRS")
            .unwrap_or_else(|_| "/etc/xdg".to_string())
            .split(':')
            .map(|d| Path::new(d).join("autostart"))
            .collect()
    }

    /// Parse un fichier .desktop minimal : Name=, Exec=, Hidden=, et
    /// l'extension GNOME X-GNOME-Autostart-enabled= (les deux mécanismes
    /// sont honorés, `Hidden=true` étant le seul défini par la spec
    /// officielle freedesktop.org, X-GNOME-Autostart-enabled étant une
    /// extension de facto largement utilisée par les outils GNOME).
    fn parse_desktop_file(path: &Path) -> Option<(String, String, bool)> {
        let content = fs::read_to_string(path).ok()?;
        let mut name = None;
        let mut exec = None;
        let mut hidden = false;
        let mut in_desktop_entry = false;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_desktop_entry = line == "[Desktop Entry]";
                continue;
            }
            if !in_desktop_entry {
                continue;
            }
            if let Some(v) = line.strip_prefix("Name=") {
                name = Some(v.to_string());
            } else if let Some(v) = line.strip_prefix("Exec=") {
                exec = Some(v.to_string());
            } else if let Some(v) = line.strip_prefix("Hidden=") {
                hidden = v.trim().eq_ignore_ascii_case("true");
            } else if let Some(v) = line.strip_prefix("X-GNOME-Autostart-enabled=") {
                if v.trim().eq_ignore_ascii_case("false") {
                    hidden = true;
                }
            }
        }

        let name = name.unwrap_or_else(|| {
            path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
        });
        let exec = exec?;
        Some((name, exec, !hidden))
    }

    pub fn fetch_entries() -> Vec<StartupEntry> {
        let user_dir = user_autostart_dir();
        let mut seen_filenames: HashMap<String, ()> = HashMap::new();
        let mut entries = Vec::new();

        // Utilisateur d'abord (prioritaire par la spec XDG en cas de doublon).
        if let Ok(read_dir) = fs::read_dir(&user_dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                if let Some(fname) = path.file_name().and_then(|f| f.to_str()) {
                    seen_filenames.insert(fname.to_string(), ());
                }
                if let Some((name, exec, enabled)) = parse_desktop_file(&path) {
                    entries.push(StartupEntry {
                        name,
                        command: exec,
                        source: StartupSource::UserAutostart,
                        enabled,
                    });
                }
            }
        }

        // Système ensuite, en ignorant les fichiers déjà surchargés côté utilisateur.
        for dir in system_autostart_dirs() {
            let Ok(read_dir) = fs::read_dir(&dir) else { continue };
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                if let Some(fname) = path.file_name().and_then(|f| f.to_str()) {
                    if seen_filenames.contains_key(fname) {
                        continue;
                    }
                }
                if let Some((name, exec, enabled)) = parse_desktop_file(&path) {
                    entries.push(StartupEntry {
                        name,
                        command: exec,
                        source: StartupSource::SystemAutostart,
                        enabled,
                    });
                }
            }
        }

        entries
    }

    fn find_user_desktop_file(name: &str) -> Option<PathBuf> {
        let dir = user_autostart_dir();
        let read_dir = fs::read_dir(&dir).ok()?;
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Some((n, _, _)) = parse_desktop_file(&path) {
                if n == name {
                    return Some(path);
                }
            }
        }
        None
    }

    fn find_system_desktop_file(name: &str) -> Option<PathBuf> {
        for dir in system_autostart_dirs() {
            let Ok(read_dir) = fs::read_dir(&dir) else { continue };
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                if let Some((n, _, _)) = parse_desktop_file(&path) {
                    if n == name {
                        return Some(path);
                    }
                }
            }
        }
        None
    }

    pub fn set_enabled_impl(name: &str, source: StartupSource, enabled: bool) -> Result<(), String> {
        let target_path = match source {
            StartupSource::UserAutostart => {
                find_user_desktop_file(name).ok_or_else(|| format!("Fichier .desktop introuvable pour \"{name}\"."))?
            }
            StartupSource::SystemAutostart => {
                // On ne modifie JAMAIS le fichier système directement (il
                // n'appartient pas à l'utilisateur, et pourrait nécessiter
                // des droits root). La spec XDG prévoit exactement ce cas
                // : une copie dans le dossier utilisateur, avec le MÊME
                // nom de fichier, prend le pas sur l'original système.
                let src = find_system_desktop_file(name)
                    .ok_or_else(|| format!("Fichier .desktop système introuvable pour \"{name}\"."))?;
                let user_dir = user_autostart_dir();
                fs::create_dir_all(&user_dir).map_err(|e| e.to_string())?;
                let fname = src.file_name().ok_or("Nom de fichier invalide")?;
                let dest = user_dir.join(fname);
                if !dest.exists() {
                    fs::copy(&src, &dest).map_err(|e| format!("Copie impossible : {e}"))?;
                }
                dest
            }
            _ => return Err("Source non applicable sur Linux.".to_string()),
        };

        let content = fs::read_to_string(&target_path).map_err(|e| e.to_string())?;
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let mut found = false;
        for line in lines.iter_mut() {
            if line.trim_start().starts_with("Hidden=") {
                *line = format!("Hidden={}", !enabled);
                found = true;
                break;
            }
        }
        if !found {
            if let Some(idx) = lines.iter().position(|l| l.trim() == "[Desktop Entry]") {
                lines.insert(idx + 1, format!("Hidden={}", !enabled));
            } else {
                lines.push(format!("Hidden={}", !enabled));
            }
        }
        fs::write(&target_path, lines.join("\n") + "\n").map_err(|e| e.to_string())
    }

    pub fn add_entry_impl(name: &str, command: &str) -> Result<(), String> {
        let dir = user_autostart_dir();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let safe_name: String =
            name.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect();
        let safe_name = if safe_name.is_empty() { "systemmonitor-entry".to_string() } else { safe_name };
        let path = dir.join(format!("{safe_name}.desktop"));
        let content = format!(
            "[Desktop Entry]\nType=Application\nName={name}\nExec={command}\nHidden=false\nX-GNOME-Autostart-enabled=true\n"
        );
        fs::write(&path, content).map_err(|e| format!("Écriture impossible : {e}"))
    }

    pub fn remove_entry_impl(name: &str, source: StartupSource) -> Result<(), String> {
        let path = match source {
            StartupSource::UserAutostart => find_user_desktop_file(name),
            _ => None,
        };
        match path {
            Some(p) => fs::remove_file(p).map_err(|e| e.to_string()),
            None => Err(
                "Seules les entrées utilisateur peuvent être supprimées (désactive les entrées système à la place)."
                    .to_string(),
            ),
        }
    }
}

// =========================================================================
// Implémentation Windows : clé Run du registre (via la crate `winreg`)
// =========================================================================
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::{StartupEntry, StartupSource};
    use winreg::enums::*;
    use winreg::{RegKey, HKCU, HKLM};

    const RUN_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    // Clé maison pour retenir les entrées désactivées par CE programme --
    // voir la note détaillée dans le commentaire de module de startup.rs :
    // Windows n'a pas de notion native documentée pour "désactiver sans
    // supprimer" une simple valeur de registre Run (contrairement à
    // StartupApproved\Run, utilisé en interne par le Gestionnaire des
    // tâches, dont le format binaire n'est pas officiellement documenté).
    // On préfère un mécanisme transparent et fiable plutôt que de le
    // reproduire à l'aveugle.
    const DISABLED_PATH: &str = r"Software\SystemMonitor\DisabledStartup";

    fn collect_from(root: &RegKey, source: StartupSource, out: &mut Vec<StartupEntry>) {
        let Ok(key) = root.open_subkey(RUN_PATH) else { return };
        let names: Vec<String> = key.enum_values().flatten().map(|(n, _)| n).collect();
        for name in names {
            if let Ok(command) = key.get_value::<String, _>(&name) {
                out.push(StartupEntry { name, command, source, enabled: true });
            }
        }
    }

    pub fn fetch_entries() -> Vec<StartupEntry> {
        let mut entries = Vec::new();
        collect_from(&HKCU, StartupSource::UserRegistry, &mut entries);
        collect_from(&HKLM, StartupSource::MachineRegistry, &mut entries);

        if let Ok(disabled_key) = HKCU.open_subkey(DISABLED_PATH) {
            let names: Vec<String> = disabled_key.enum_values().flatten().map(|(n, _)| n).collect();
            for name in names {
                if let Ok(command) = disabled_key.get_value::<String, _>(&name) {
                    entries.push(StartupEntry {
                        name,
                        command,
                        source: StartupSource::UserRegistry,
                        enabled: false,
                    });
                }
            }
        }
        entries
    }

    pub fn set_enabled_impl(name: &str, source: StartupSource, enabled: bool) -> Result<(), String> {
        let root = match source {
            StartupSource::UserRegistry => &HKCU,
            StartupSource::MachineRegistry => &HKLM,
            _ => return Err("Source non applicable sur Windows.".to_string()),
        };

        if enabled {
            // Ré-active : lit la commande depuis notre magasin "disabled"
            // maison, la remet dans Run, puis nettoie l'entrée désactivée.
            let disabled_key = HKCU
                .open_subkey_with_flags(DISABLED_PATH, KEY_ALL_ACCESS)
                .map_err(|e| format!("Impossible d'ouvrir le magasin des entrées désactivées : {e}"))?;
            let command: String = disabled_key
                .get_value(name)
                .map_err(|_| format!("Entrée désactivée \"{name}\" introuvable."))?;

            let run_key = root
                .open_subkey_with_flags(RUN_PATH, KEY_ALL_ACCESS)
                .map_err(|e| format!("Accès à la clé Run refusé : {e}"))?;
            run_key.set_value(name, &command).map_err(|e| format!("Écriture impossible : {e}"))?;

            let _ = disabled_key.delete_value(name);
        } else {
            // Désactive : déplace la commande de Run vers notre magasin
            // "disabled" maison (pour pouvoir la restaurer plus tard),
            // puis retire la valeur de Run.
            let run_key = root
                .open_subkey_with_flags(RUN_PATH, KEY_ALL_ACCESS)
                .map_err(|e| format!("Accès à la clé Run refusé : {e}"))?;
            let command: String =
                run_key.get_value(name).map_err(|e| format!("Entrée \"{name}\" introuvable dans Run : {e}"))?;

            let (disabled_key, _) = HKCU
                .create_subkey(DISABLED_PATH)
                .map_err(|e| format!("Impossible de créer le magasin des entrées désactivées : {e}"))?;
            disabled_key.set_value(name, &command).map_err(|e| format!("Écriture impossible : {e}"))?;

            run_key.delete_value(name).map_err(|e| format!("Suppression impossible : {e}"))?;
        }
        Ok(())
    }

    pub fn add_entry_impl(name: &str, command: &str) -> Result<(), String> {
        // Toujours en scope UTILISATEUR (HKCU), jamais HKLM -- cohérent
        // avec la règle du reste de ce projet : ne jamais exiger de
        // droits admin pour une action que l'utilisateur peut faire sur
        // son propre compte.
        let run_key = HKCU
            .open_subkey_with_flags(RUN_PATH, KEY_ALL_ACCESS)
            .or_else(|_| HKCU.create_subkey(RUN_PATH).map(|(k, _)| k))
            .map_err(|e| format!("Accès à la clé Run refusé : {e}"))?;
        run_key.set_value(name, &command).map_err(|e| format!("Écriture impossible : {e}"))
    }

    pub fn remove_entry_impl(name: &str, source: StartupSource) -> Result<(), String> {
        match source {
            StartupSource::UserRegistry => {
                let run_key = HKCU
                    .open_subkey_with_flags(RUN_PATH, KEY_ALL_ACCESS)
                    .map_err(|e| format!("Accès à la clé Run refusé : {e}"))?;
                let _ = run_key.delete_value(name); // peut déjà être dans "disabled"
                if let Ok(disabled_key) = HKCU.open_subkey_with_flags(DISABLED_PATH, KEY_ALL_ACCESS) {
                    let _ = disabled_key.delete_value(name);
                }
                Ok(())
            }
            _ => Err(
                "Seules les entrées HKCU (utilisateur) peuvent être supprimées via cette application."
                    .to_string(),
            ),
        }
    }
}
