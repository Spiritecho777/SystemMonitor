use std::process::Command;

#[derive(Clone, Debug)]
pub struct ServiceRow {
    pub name: String,
    pub state: String, // ex: "active (running)" sur Linux, "RUNNING" sur Windows
    pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
}

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

    pub fn control(&self, name: &str, action: ServiceAction) -> Result<(), String> {
        control_service(name, action)
    }
}

fn command_no_window(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(target_os = "linux")]
fn fetch_services() -> Vec<ServiceRow> {
    let output = match command_no_window("systemctl")
        .args(["list-units", "--type=service", "--all", "--plain", "--no-legend", "--no-pager"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let unit = it.next()?;
            let _load = it.next()?;
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
    run_and_collect("systemctl", &[verb, name])
}

#[cfg(target_os = "windows")]
fn fetch_services() -> Vec<ServiceRow> {
    let output = match command_no_window("sc").args(["query", "type=", "service", "state=", "all"]).output() {
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

fn run_and_collect(cmd: &str, args: &[&str]) -> Result<(), String> {
    let output = command_no_window(cmd)
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
