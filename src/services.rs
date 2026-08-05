use std::process::Command;

/// Une ligne de service système.
///
/// Le concept de "service" est fondamentalement différent entre les deux
/// plateformes -- systemd (Linux) expose des centaines d'unités y compris
/// des sockets/timers/devices non pertinents ici, tandis que le Service
/// Control Manager (Windows) est plus proche de ce qu'affiche l'onglet
/// "Services" du Gestionnaire des tâches natif. Les commandes utilisées
/// et le parsing sont donc entièrement séparés par plateforme.
#[derive(Clone, Debug)]
pub struct ServiceRow {
    pub name: String,
    pub state: String, // ex: "active (running)" sur Linux, "RUNNING" sur Windows
    pub description: String,
}

/// Action de contrôle demandée sur un service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
}

/// IMPORTANT -- limitation connue : `refresh()` exécute une commande
/// externe (`systemctl`/`sc`) de façon SYNCHRONE sur le thread principal
/// de l'UI. `systemctl list-units` est généralement rapide (<100ms), mais
/// `sc query type= service state= all` peut prendre plus longtemps selon
/// le nombre de services installés sur la machine Windows. Pour limiter
/// l'impact, `AppState` (voir state.rs) n'appelle ce refresh() qu'une
/// fois toutes les quelques itérations, pas à chaque tick.
///
/// IMPORTANT -- privilèges requis pour `control()` : démarrer, arrêter ou
/// redémarrer un service nécessite des privilèges élevés (root sur
/// Linux, administrateur sur Windows). Ce binaire ne tourne avec AUCUN
/// privilège spécial par défaut -- `control()` tentera l'action telle
/// quelle et remontera l'erreur système brute si elle échoue (message de
/// type "Access denied"/"Interactive authentication required" côté
/// Windows, ou le contenu stderr de systemctl côté Linux), plutôt que de
/// tenter une élévation automatique (UAC/pkexec), volontairement hors
/// scope pour cette première itération.
pub struct ServiceMonitor {
    services: Vec<ServiceRow>,
}

impl ServiceMonitor {
    pub fn new() -> Self {
        let mut s = Self { services: Vec::new() };
        s.refresh();
        s
    }

    pub fn refresh(&mut self) {
        self.services = fetch_services();
    }

    pub fn services(&self) -> &[ServiceRow] {
        &self.services
    }

    /// Tente d'exécuter `action` sur le service `name`. Retourne `Ok(())`
    /// si la commande système a réussi (code de sortie 0), sinon `Err`
    /// avec un message combinant stdout/stderr pour affichage direct
    /// dans l'UI -- volontairement pas de traduction/simplification du
    /// message d'erreur système, pour ne pas masquer la vraie cause
    /// (permissions, service inconnu, etc.) à l'utilisateur.
    pub fn control(&self, name: &str, action: ServiceAction) -> Result<(), String> {
        control_service(name, action)
    }
}

#[cfg(target_os = "linux")]
fn fetch_services() -> Vec<ServiceRow> {
    // --plain --no-legend : sortie stable, une ligne par service, sans
    // en-tête ni mise en forme -- format documenté de longue date et
    // recommandé pour un parsing fiable.
    let output = match Command::new("systemctl")
        .args(["list-units", "--type=service", "--all", "--plain", "--no-legend", "--no-pager"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(), // systemctl absent (non-systemd) : liste vide plutôt que planter
    };

    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let unit = it.next()?;
            let _load = it.next()?; // consommé mais non affiché
            let active = it.next()?;
            let sub = it.next()?;
            let description: String = it.collect::<Vec<_>>().join(" ");
            Some(ServiceRow {
                name: unit.trim_end_matches(".service").to_string(),
                state: format!("{active} ({sub})"),
                description,
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn control_service(name: &str, action: ServiceAction) -> Result<(), String> {
    let verb = match action {
        ServiceAction::Start => "start",
        ServiceAction::Stop => "stop",
        ServiceAction::Restart => "restart",
    };
    // Le nom stocké dans ServiceRow a déjà eu son suffixe ".service"
    // retiré (voir fetch_services ci-dessus) -- systemctl l'accepte très
    // bien sans ce suffixe, il n'est donc pas nécessaire de le rajouter.
    run_and_collect("systemctl", &[verb, name])
}

#[cfg(target_os = "windows")]
fn fetch_services() -> Vec<ServiceRow> {
    // `sc query` renvoie un texte en blocs, chaque bloc commençant par
    // une ligne "SERVICE_NAME: ...". On parcourt ligne par ligne en
    // repérant les champs par leur clé exacte (avant le ':'), plutôt que
    // par position -- plus robuste face aux variations de largeur de
    // colonnes entre versions de Windows.
    let output = match Command::new("sc").args(["query", "type=", "service", "state=", "all"]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut services = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_display: Option<String> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        let Some(idx) = line.find(':') else { continue };
        let (key, rest) = line.split_at(idx);
        let key = key.trim();
        let value = rest[1..].trim();

        match key {
            "SERVICE_NAME" => current_name = Some(value.to_string()),
            "DISPLAY_NAME" => current_display = Some(value.to_string()),
            "STATE" => {
                if let Some(name) = current_name.take() {
                    let state = value.split_whitespace().nth(1).unwrap_or(value).to_string();
                    let description = current_display.take().unwrap_or_else(|| name.clone());
                    services.push(ServiceRow { name, state, description });
                }
            }
            _ => {}
        }
    }
    services
}

#[cfg(target_os = "windows")]
fn control_service(name: &str, action: ServiceAction) -> Result<(), String> {
    // `sc` n'a pas de sous-commande "restart" native -- on l'implémente
    // en enchaînant stop puis start. Un échec du stop (ex: service déjà
    // arrêté) n'empêche pas de tenter le start qui suit : l'objectif
    // final ("le service tourne") prime sur la réussite de chaque étape
    // individuelle.
    match action {
        ServiceAction::Start => run_and_collect("sc", &["start", name]),
        ServiceAction::Stop => run_and_collect("sc", &["stop", name]),
        ServiceAction::Restart => {
            let _ = run_and_collect("sc", &["stop", name]);
            run_and_collect("sc", &["start", name])
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn fetch_services() -> Vec<ServiceRow> {
    Vec::new()
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn control_service(_name: &str, _action: ServiceAction) -> Result<(), String> {
    Err("Contrôle de service non supporté sur cette plateforme.".to_string())
}

/// Exécute une commande et transforme son résultat en `Result` exploitable
/// par l'UI : succès si code de sortie 0, sinon le contenu de
/// stdout+stderr (souvent stderr contient le vrai message d'erreur côté
/// systemctl/sc, ex: "Access denied", "Unit not found").
fn run_and_collect(cmd: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Impossible de lancer {cmd} : {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let mut msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if msg.is_empty() {
            msg = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        if msg.is_empty() {
            msg = format!("{cmd} a échoué (code {:?})", output.status.code());
        }
        Err(msg)
    }
}
