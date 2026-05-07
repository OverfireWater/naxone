<div align="center">

<img src="logo_transparent.png" alt="NaxOne" width="160" />

# NaxOne

**One-stop local development environment** · Rewritten in Rust, packaged with Tauri, modern UI in Vue

[![License: MIT](https://img.shields.io/github/license/OverfireWater/naxone?color=blue)](LICENSE)
[![GitHub release](https://img.shields.io/github/v/release/OverfireWater/naxone?label=release&color=brightgreen)](https://github.com/OverfireWater/naxone/releases)
[![GitHub downloads](https://img.shields.io/github/downloads/OverfireWater/naxone/total?label=downloads&color=orange)](https://github.com/OverfireWater/naxone/releases)
[![GitHub stars](https://img.shields.io/github/stars/OverfireWater/naxone?style=flat&label=GitHub%20stars)](https://github.com/OverfireWater/naxone/stargazers)
[![Gitee stars](https://gitee.com/kz_y/naxone/badge/star.svg?theme=dark)](https://gitee.com/kz_y/naxone/stargazers)
[![Gitee forks](https://gitee.com/kz_y/naxone/badge/fork.svg?theme=dark)](https://gitee.com/kz_y/naxone/members)

[Download (Gitee)](https://gitee.com/kz_y/naxone/releases/latest) · [Download (GitHub)](https://github.com/OverfireWater/naxone/releases/latest) · [Issues](https://github.com/OverfireWater/naxone/issues)

[中文版 README](README.md) · **English**

</div>

---

NaxOne is a Windows local development environment manager built for PHP developers. A single native desktop app puts Nginx, Apache, MySQL, Redis, and multiple PHP versions in one panel — start/stop, configuration, virtual hosts, and SSL all handled in one place.

Stack: **Rust + Tauri 2 + Vue 3 + TypeScript**. Cold start < 1s, resident memory < 100MB, installer just 4MB.

> **How does it relate to PHPStudy?** NaxOne **does not depend on PHPStudy** — it can be installed and run standalone. But if PHPStudy is **already on your machine**, NaxOne automatically detects its install directory and PHP/Nginx/MySQL packages, bringing them under management without reinstalling and without breaking existing sites. The two coexist peacefully.

## Highlights

- **Service management**: One-click start/stop for Nginx / Apache / MySQL / Redis / PHP-CGI; Nginx and Apache are auto-mutually-exclusive; starting a web server auto-launches PHP-CGI; port probing + process name verification means no false positives.
- **Virtual hosts**: Create / edit / delete sites; **dual-write** to both Nginx and Apache configs (zero cost to switch engines); auto-update `hosts` file; instant reload on save; rewrite presets (Laravel / ThinkPHP / WordPress); one-click self-signed SSL.
- **Multiple PHP versions**: Install many PHP versions locally, each site picks its own; one-click switch the global CLI `php` command (a shim is mounted on user PATH, takes effect in any new terminal).
- **Service configuration**: Visual editor for 19 Nginx / 25 MySQL / 20 Redis / 34 PHP options; PHP extension toggles; `.bak` backup before every change.
- **Software store**: Built-in PHP official source + GitHub mirror, on-demand download of historical versions, multi-mirror acceleration.
- **Stranger process detection**: Dashboard automatically identifies external processes occupying ports 80/3306/6379 (including PHPStudy services) — kill with one click.
- **Modern experience**: Compact dark UI with glassmorphism, no native title bar, system tray minimization, auto update check, memory monitoring.

## Screenshots

<table>
<tr>
<td width="50%"><b>Dashboard</b><br/>Glassmorphism + ambient orbs. One-click start/stop for Nginx/Apache/MySQL/Redis. Multi-version PHP managed in one row<br/><img src="docs/screenshots/01-dashboard.png" alt="Dashboard" /></td>
<td width="50%"><b>New Site</b><br/>Three tabs: Basic / Rewrite preset / SSL & advanced<br/><img src="docs/screenshots/03-vhosts-modal-basic.png" alt="New site" /></td>
</tr>
<tr>
<td><b>Software Store</b><br/>Built-in multi-version downloads for PHP/Nginx/MySQL/etc., multi-mirror acceleration, SHA-256 verified<br/><img src="docs/screenshots/06-store.png" alt="Store" /></td>
<td><b>Service Config · Nginx</b><br/>19 common Nginx options visualized, automatic <code>.bak</code> before write<br/><img src="docs/screenshots/08-config-nginx.png" alt="Nginx config" /></td>
</tr>
<tr>
<td><b>Global Environment</b><br/>One-click switch global CLI <code>php</code> / <code>composer</code> / <code>node</code> / <code>mysql</code> versions<br/><img src="docs/screenshots/07-config-env.png" alt="Global env" /></td>
<td><b>Settings</b><br/>PHPStudy path / WWW root / ports / auto-start / theme<br/><img src="docs/screenshots/13-settings.png" alt="Settings" /></td>
</tr>
</table>

> More screenshots in [docs/screenshots/](docs/screenshots/) (13 total, covers every page).

## Install

Download the latest installer:

| | Link |
|---|---|
| China (recommended) | [Gitee Releases](https://gitee.com/kz_y/naxone/releases/latest) |
| Worldwide | [GitHub Releases](https://github.com/OverfireWater/naxone/releases/latest) |

File name `NaxOne_X.Y.Z_x64-setup.exe`, around 4 MB. NSIS installer, **defaults to `D:\NaxOne`** (falls back to `C:\NaxOne` if D: doesn't exist).

Requires Windows 10 1809+ / Windows 11, x64.

### First run: Windows SmartScreen warning

The first time you run it, Windows may show:

> Windows protected your PC
> Microsoft Defender SmartScreen prevented an unrecognized app from starting…

How to bypass (any of):

1. In the popup click **More info** → **Run anyway**
2. Right-click the .exe → **Properties** → check **Unblock** → OK
3. PowerShell: `Unblock-File -Path "D:\NaxOne\NaxOne.exe"`

## Build from source

### Prerequisites

- [Rust](https://rustup.rs/) >= 1.75
- [Node.js](https://nodejs.org/) >= 20

### Dev mode

```bash
# Install frontend deps
cd crates/naxone-tauri/frontend
npm install

# Run (Vite + Rust compile + Tauri window all in one)
cd ..
cargo tauri dev
```

### Package

```bash
cargo tauri build
# Installer at target/release/bundle/nsis/
```

### Tests

```bash
cargo test --workspace
```

## Architecture

**Hexagonal architecture** (Ports & Adapters) — core business logic is fully decoupled from external dependencies.

![Architecture](docs/diagrams/architecture.png)

- **`naxone-core`**: Pure domain logic, zero external deps, fully unit-testable.
- **`naxone-adapters`**: Implements the port traits defined in core, swappable (a future Linux TUI just replaces this layer).
- **`naxone-tauri`**: Only IPC forwarding and app initialization.

### Service start flow (with mutex + cascade)

![Service flow](docs/diagrams/service-flow.png)

### Vhost dual-write flow (Nginx + Apache in sync)

![Vhost flow](docs/diagrams/vhost-flow.png)

## Project layout

```
naxone/
├── Cargo.toml                          # Workspace root (unified version, shared deps)
├── LICENSE                             # MIT
├── logo.png / logo_transparent.png     # Brand assets
├── crates/
│   ├── naxone-core/                    # Pure domain logic, zero external deps
│   │   └── src/
│   │       ├── domain/                 # Domain models: Service / VirtualHost / PHP / Log
│   │       ├── ports/                  # Port traits: ProcessManager / ConfigIO / TemplateEngine / PlatformOps
│   │       ├── use_cases/              # Use cases: ServiceManager / VhostManager / PhpManager / ConfigEditor
│   │       ├── config.rs               # AppConfig (TOML config deserialization)
│   │       └── error.rs                # Unified error type
│   │
│   ├── naxone-adapters/                # Concrete implementations of the ports
│   │   └── src/
│   │       ├── config/                 # FsConfigIO (file IO)
│   │       ├── package/                # Package scanner + store (PHP official / GitHub mirror)
│   │       ├── platform/               # WindowsPlatform / LinuxPlatform (hosts, self-signed SSL, global PHP shim)
│   │       ├── process/                # NativeProcessManager (process start/stop + port probing)
│   │       ├── template/               # SimpleTemplateEngine (generates nginx/apache vhost configs)
│   │       └── vhost/                  # VhostScanner (parses existing virtual hosts)
│   │
│   └── naxone-tauri/                   # Desktop app shell
│       ├── src/
│       │   ├── main.rs                 # Tauri entry (system tray, plugin registration)
│       │   ├── state.rs                # AppState (DI, config loading, legacy user migration)
│       │   └── commands/               # Tauri IPC commands: service / vhost / php / settings / package / updater ...
│       ├── frontend/                   # Vue 3 + Vite + Tailwind frontend
│       │   └── src/
│       │       ├── App.vue             # Root layout (custom title bar, sidebar, routing)
│       │       ├── views/              # Pages: Dashboard / Vhosts / ServiceConfig / Settings
│       │       ├── components/         # Reusable components: StoreCard / LogDrawer / SelectMenu ...
│       │       └── assets/             # global.css (Tailwind + theme variables)
│       ├── tauri.conf.json             # Tauri config
│       ├── nsis/installer-hooks.nsh    # Windows installer customization (defaults to D:\NaxOne)
│       ├── icons/                      # App icons (auto-generated from logo.png)
│       └── capabilities/               # Tauri permission config
```

## Compatibility

- **PHPStudy Pro**: Auto-scans its `Extensions` directory for PHP/Nginx/Apache/MySQL/Redis packages; the vhost config format generated is identical to PHPStudy's, allowing two-way migration.
- **Official PHP packages**: Directly recognizes zip archives extracted from windows.php.net.
- **Config files**: Every write operation creates a `.bak` backup first.
- **Migrating from RustStudy**: On first launch, NaxOne automatically migrates `~/.ruststudy` and `%APPDATA%\RustStudy` to the corresponding NaxOne directories.

## License

[MIT](LICENSE) © 2026 NaxOne Contributors

The managed binaries (PHP / Nginx / Apache / MySQL / Redis) follow their respective original licenses (PHP License / BSD / Apache 2.0 / GPL / BSD). NaxOne only acts as a local launcher and config tool — it does **not** redistribute these binaries.
