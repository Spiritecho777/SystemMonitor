# SystemMonitor

A cross-platform task manager (Linux / Windows) written in Rust with [FLTK](https://www.fltk.org/), inspired by the Windows Task Manager but designed to run just as well on a Linux desktop as on Windows, including in a remote (RDP) session without GPU acceleration.

Completely vibecoder.

## Features

- **Processes (Details tab)** — sortable (click on column headers) and filterable list by name, showing CPU %, memory, PID, and status. Columns are resizable by drag-and-drop and automatically adapt to window size.
- **Services** — list of system services (`systemd` on Linux, Service Control Manager on Windows), with search by name/description and **Start / Stop / Restart** actions.
- **Disks** — used/total space per volume, plus a real-time read/write throughput (calculated over the actual elapsed interval, not a fixed nominal one).
- **Startup** — list of applications launched at session startup (XDG `.desktop` files on Linux, `Run` registry key on Windows), with the ability to **enable/disable**, **add**, or **remove** an entry.
- **Run a task** — launches any executable or command, just like double-clicking it in a file explorer.
- **CPU and temperature history** — two annotated graphs (current value, observed peak, scale bounds) updated continuously.
- **Dark theme** — Dracula palette combined with the `Fluent` rendering scheme from [fltk-theme](https://github.com/fltk-rs/fltk-theme), for a clean, modern look rather than FLTK's default styling.

## Architecture

The project is split by responsibility rather than keeping all the logic in a single file:

| File | Role |
|---|---|
| `main.rs` | Entry point: window construction, tab assembly, callback wiring, main loop. |
| `ui_style.rs` | Style and geometry constants (colors, sizes, tab positioning). |
| `ui_shared.rs` | State shared across callbacks (`Shared`): sorting, filters, current selection. |
| `ui_widgets.rs` | Generic drawing functions (column headers, zebra rows, graphs) and column width management. |
| `ui_tabs.rs` | Construction of each tab (Details / Services / Disks / Startup). |
| `ui_callbacks.rs` | Event wiring (clicks, sorting, filters, action buttons) and periodic refresh. |
| `state.rs` | Aggregation of all monitors (`AppState`) and CPU/temperature history. |
| `process_monitor.rs` | Reading processes and CPU/RAM usage via [`sysinfo`](https://docs.rs/sysinfo). |
| `temperature.rs` | Reading temperature sensors (`sysinfo::Components`). |
| `disks.rs` | Reading disk space and computing read/write throughput. |
| `services.rs` | Querying and controlling services (`systemctl` / `sc.exe`), with console window suppression on Windows (`CREATE_NO_WINDOW`). |
| `startup.rs` | Managing startup applications (`.desktop` files / Windows registry via `winreg`). |
| `launcher.rs` | Detached execution of an arbitrary command. |

## Building

### Windows

```powershell
cargo build --release
```

The binary is produced directly — `fltk-bundled` downloads a prebuilt FLTK bundle, so no additional system dependencies are required.

### Linux (x86_64 and aarch64), from WSL

A `Deploy.ps1` script orchestrates cross-compilation from Windows via WSL (AlmaLinux distribution):

```powershell
.\Deploy.ps1                  # builds both x86_64 and arm64
.\Deploy.ps1 -Target x86_64    # single architecture only
.\Deploy.ps1 -NoSync           # skips re-syncing the code to WSL
```

Key points of this build chain:
- The code is **synced to WSL's native filesystem** (not `/mnt/c/...`) before building: `fltk-bundled` extracts its bundle via `tar`, which fails on Windows' DrvFs mount (`Cannot utime: Operation not permitted`).
- The `aarch64` target is built via [`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild), which bundles its own glibc sysroot — no need for an RHEL cross-toolchain.
- A dedicated ARM64 sysroot (X11/Pango/Cairo/fontconfig) is built once via `dnf --forcearch=aarch64 --installroot=...`, required for FLTK's final link step on that architecture.

### Linux distribution: tar.gz + install script

No AppImage: `install.sh` / `uninstall.sh` detect the distribution (Arch, Debian/Ubuntu, Fedora, openSUSE) and install the required runtime dependencies (X11, Pango, Cairo, fontconfig) using each distro's own package names.

Killing processes owned by other users relies on the Linux `CAP_KILL` capability (`setcap cap_kill=+ep`), applied by `install.sh`, no permanent root elevation.

## Known limitations

- **Temperature**: entirely dependent on what the operating system exposes.
  - Absent in a VM (WSL, VirtualBox, etc.) — the virtualized kernel doesn't expose any physical sensors.
  - On Windows, depends on the motherboard/BIOS's WMI implementation, which is rarely complete.
  - On bare-metal Linux (Intel `coretemp`, AMD `k10temp`), works natively with no configuration needed.
- **Service control** (Start/Stop/Restart) requires elevated privileges (root on Linux, administrator on Windows). No automatic elevation is implemented: on Linux, authentication is delegated to **polkit** (native desktop session behavior); on Windows, the action simply fails with the system error message if the app isn't running as administrator.
- **Windows startup management**: unlike the native "Startup Apps" screen in Windows (which uses an undocumented internal format, `StartupApproved\Run`), disabling an entry here moves its value into a backup registry key owned by this application. Functionally equivalent, but not recognized by Windows' native tool.
- Drag-to-resize columns requires `Fl_Table`'s native header (`ColHeader`): header styling is therefore drawn directly within that context rather than via separate widgets.

## Main dependencies

- [`fltk`](https://crates.io/crates/fltk) (`fltk-bundled` feature) — graphical interface.
- [`fltk-theme`](https://crates.io/crates/fltk-theme) — dark theme and Fluent rendering.
- [`sysinfo`](https://crates.io/crates/sysinfo) — processes, memory, disks, temperature.
- [`winreg`](https://crates.io/crates/winreg) (Windows only) — registry read/write for startup management.
