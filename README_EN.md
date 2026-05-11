<div align="center">

<img src="logo_transparent.png" alt="NaxOne" width="160" />

# NaxOne

**One-stop local dev environment for Windows** · Rust + Tauri 2 + Vue 3

[![License: MIT](https://img.shields.io/github/license/OverfireWater/naxone?color=blue)](LICENSE)
[![GitHub release](https://img.shields.io/github/v/release/OverfireWater/naxone?label=release&color=brightgreen)](https://github.com/OverfireWater/naxone/releases)
[![GitHub downloads](https://img.shields.io/github/downloads/OverfireWater/naxone/total?label=downloads&color=orange)](https://github.com/OverfireWater/naxone/releases)
[![GitHub stars](https://img.shields.io/github/stars/OverfireWater/naxone?style=flat&label=GitHub%20stars)](https://github.com/OverfireWater/naxone/stargazers)
[![Gitee stars](https://gitee.com/kz_y/naxone/badge/star.svg?theme=dark)](https://gitee.com/kz_y/naxone/stargazers)

[Download from Gitee](https://gitee.com/kz_y/naxone/releases/latest) · [Download from GitHub](https://github.com/OverfireWater/naxone/releases/latest) · [Issues](https://github.com/OverfireWater/naxone/issues) · [中文](README.md)

</div>

---

A Windows desktop app built for PHP developers. **One panel** to manage Nginx / Apache / MySQL / Redis / multi-version PHP — services, virtual hosts, SSL, config editing, one-click site templates, all in one place.

Cold start < 1s · Resident memory < 100MB · Installer 6MB · Defaults to `D:\NaxOne`

> 💡 **Coexists with PHPStudy**: NaxOne does not depend on PHPStudy and runs standalone. If PHPStudy is already on your machine, NaxOne auto-detects its PHP/Nginx/MySQL packages and brings them under management — no reinstall, no breaking existing sites.

## 📦 Install

Download the latest installer for your region:

| Region | Link |
|---|---|
| China (recommended) | [Gitee Releases](https://gitee.com/kz_y/naxone/releases/latest) |
| Worldwide | [GitHub Releases](https://github.com/OverfireWater/naxone/releases/latest) |

Requires Windows 10 1809+ / Windows 11, x64.

<details>
<summary>How to bypass the Windows SmartScreen warning on first run?</summary>

NaxOne doesn't ship with a paid code-signing certificate, so Windows may say "unrecognized app". Any of these:

1. In the popup, click **More info** → **Run anyway**
2. Right-click the .exe → **Properties** → check **Unblock** → OK
3. PowerShell: `Unblock-File -Path "D:\NaxOne\NaxOne.exe"`

Only needed on first run.
</details>

## 🖼 Screenshots

<table>
<tr>
<td width="50%"><a href="docs/screenshots/01-dashboard.png"><img src="docs/screenshots/01-dashboard.png" alt="Dashboard" /></a><br><b>Dashboard</b>: one-row start/stop for Nginx/Apache/MySQL/Redis, PHP multi-version, recent activity log</td>
<td width="50%"><a href="docs/screenshots/03-vhosts-modal-basic.png"><img src="docs/screenshots/03-vhosts-modal-basic.png" alt="New site" /></a><br><b>New site</b>: Basic / Rewrite / SSL tabs; one-click template install; auto-signed HTTPS</td>
</tr>
<tr>
<td width="50%"><a href="docs/screenshots/08-config-nginx.png"><img src="docs/screenshots/08-config-nginx.png" alt="Service config" /></a><br><b>Service config</b>: 19 Nginx / 25 MySQL / 20 Redis / 34 PHP options, all visual</td>
<td width="50%"><a href="docs/screenshots/06-store.png"><img src="docs/screenshots/06-store.png" alt="Store" /></a><br><b>Software store</b>: multi-version PHP/Nginx/MySQL download with multi-mirror acceleration</td>
</tr>
</table>

> Full 13 screenshots in [docs/screenshots/](docs/screenshots/)

## ✨ Highlights

- 🚀 **Service management** — one-click start/stop; Nginx/Apache mutex; auto-launch PHP-CGI; port probing + process verification, no false positives
- 🌐 **Virtual hosts** — dual-write to both Nginx + Apache configs; auto `hosts` file; one-click mkcert HTTPS (green lock directly in the browser)
- 📦 **Site templates** — pick WordPress / Laravel / ThinkPHP / Webman / Blank when creating a vhost, auto-downloads or runs `composer create-project`
- 🐘 **Multi-version PHP** — per-site PHP version; one-click switch global CLI `php`; same for Composer / Node / MySQL
- ⚙️ **Visual config** — hundreds of Nginx/MySQL/Redis/PHP options visually editable, auto `.bak` before write
- 📋 **Activity log** — every write op archived; keyword search, category filter, error rows highlighted, full stdout reviewable after modal close
- 🏪 **Software store** — PHP official source + GitHub mirror, on-demand historical version download, SHA-256 verified
- 🔍 **Port diagnosis** — auto-identify external processes occupying 80/3306/6379 (incl. PHPStudy services), kill with one click

<details>
<summary>More details</summary>

- **mkcert SSL**: auto-create local CA and install to current user cert store (no admin needed), browser trusts directly
- **PHP extensions**: powered by [PIE](https://github.com/php/pie), one-click install/uninstall; auto-picks PHP 8.1+ as runtime
- **Numeric validation**: pre-save range checks (e.g. `gzip_level` 1-9), avoiding `nginx reload` emerg errors
- **Webman template**: auto-configures `proxy_pass http://127.0.0.1:8787`, reminds you to start the cli via `windows.bat`
- **Tauri auto-updater**: built-in update check, signed update flow, seamless upgrade
- **Modern UX**: light/dark theme toggle, compact UI, no native title bar, tray minimize

</details>

## 🤝 Compatibility

| Scenario | Notes |
|---|---|
| **PHPStudy Pro** | Auto-scans its `Extensions` directory for PHP/Nginx/Apache/MySQL/Redis; vhost config format is identical to PHPStudy's, allowing two-way migration |
| **Official PHP packages** | Directly recognizes zip archives extracted from windows.php.net |
| **Config files** | Every write op creates a `.bak` backup first |
| **Portable** | All data lives in `%USERPROFILE%\.naxone\` — copy this single folder to migrate machines |

## 🛠 Build from source

Requires [Rust](https://rustup.rs/) >= 1.75 and [Node.js](https://nodejs.org/) >= 20.

```bash
# Install frontend deps
cd crates/naxone-tauri/frontend && npm install && cd ..

# Dev mode (Vite + Rust + Tauri all in one)
cargo tauri dev

# Build NSIS installer (output at target/release/bundle/nsis/)
cargo tauri build

# Tests
cargo test --workspace
```

## 🏗 Architecture

**Hexagonal architecture** (Ports & Adapters) — core business logic is fully decoupled from external deps:

- **`naxone-core`** — pure domain logic, zero external deps, fully unit-testable
- **`naxone-adapters`** — implements the port traits (process management, file IO, template rendering, platform API), future cross-platform expansion just replaces this layer
- **`naxone-tauri`** — desktop app shell (Tauri IPC + Vue frontend + app init)

![Architecture](docs/diagrams/architecture.png)

<details>
<summary>Full project layout</summary>

```
naxone/
├── Cargo.toml                          # Workspace root
├── LICENSE                             # MIT
├── crates/
│   ├── naxone-core/                    # Pure domain logic
│   │   └── src/
│   │       ├── domain/                 # Domain models: Service / VirtualHost / PHP / Log
│   │       ├── ports/                  # Port traits: ProcessManager / ConfigIO / TemplateEngine / PlatformOps
│   │       ├── use_cases/              # Use cases: ServiceManager / VhostManager / PhpManager / ConfigEditor
│   │       ├── config.rs               # AppConfig
│   │       └── error.rs                # Unified error type
│   │
│   ├── naxone-adapters/                # Concrete implementations of the ports
│   │   └── src/
│   │       ├── config/                 # FsConfigIO
│   │       ├── package/                # Package scanner + store
│   │       ├── platform/               # WindowsPlatform (hosts, self-signed SSL, global PHP shim)
│   │       ├── process/                # NativeProcessManager
│   │       ├── template/               # SimpleTemplateEngine
│   │       └── vhost/                  # VhostScanner
│   │
│   └── naxone-tauri/                   # Desktop app shell
│       ├── src/                        # Tauri entry + IPC commands
│       ├── frontend/                   # Vue 3 + Vite + Tailwind
│       │   └── src/views/              # Dashboard / Vhosts / ServiceConfig / GlobalEnv / Settings
│       ├── tauri.conf.json
│       ├── nsis/installer-hooks.nsh    # NSIS installer customization (defaults to D:\NaxOne)
│       └── icons/                      # App icons
```

</details>

<details>
<summary>Service start / vhost dual-write flow diagrams</summary>

### Service start flow (with mutex + cascade)
![Service flow](docs/diagrams/service-flow.png)

### Vhost dual-write flow (Nginx + Apache in sync)
![Vhost flow](docs/diagrams/vhost-flow.png)

</details>

## 📜 License

[MIT](LICENSE) © 2026 NaxOne Contributors

The managed binaries (PHP / Nginx / Apache / MySQL / Redis) follow their respective original licenses (PHP License / BSD / Apache 2.0 / GPL / BSD). NaxOne is purely a local launcher and configuration tool — it does **not** redistribute these binaries.
