<div align="center">

<img src="logo_transparent.png" alt="NaxOne" width="160" />

# NaxOne

**Windows 本地开发集成环境** · Rust + Tauri 2 + Vue 3

[![License: MIT](https://img.shields.io/github/license/OverfireWater/naxone?color=blue)](LICENSE)
[![GitHub release](https://img.shields.io/github/v/release/OverfireWater/naxone?label=release&color=brightgreen)](https://github.com/OverfireWater/naxone/releases)
[![GitHub downloads](https://img.shields.io/github/downloads/OverfireWater/naxone/total?label=downloads&color=orange)](https://github.com/OverfireWater/naxone/releases)
[![GitHub stars](https://img.shields.io/github/stars/OverfireWater/naxone?style=flat&label=GitHub%20stars)](https://github.com/OverfireWater/naxone/stargazers)
[![Gitee stars](https://gitee.com/kz_y/naxone/badge/star.svg?theme=dark)](https://gitee.com/kz_y/naxone/stargazers)

[Gitee 下载](https://gitee.com/kz_y/naxone/releases/latest) · [GitHub 下载](https://github.com/OverfireWater/naxone/releases/latest) · [问题反馈](https://gitee.com/kz_y/naxone/issues) · [English](README_EN.md)

</div>

---

面向 PHP 开发者的 Windows 桌面 App。**一个面板**把 Nginx / Apache / MySQL / Redis / 多版本 PHP 全部管起来 —— 服务启停、虚拟主机、SSL、配置编辑、站点模板装包，一站搞定。

冷启动 < 1s · 内存常驻 < 100MB · 安装包 6MB · 默认装到 `D:\NaxOne`

> 💡 **跟 PHPStudy 共存**：NaxOne 不依赖 PHPStudy，独立可用；但如果你机器上有 PHPStudy，NaxOne 会自动识别它的 PHP/Nginx/MySQL 包并纳入管理，无需重装、不破坏现有站点。

## 📦 安装

下载对应平台安装包，双击即可：

| 渠道 | 链接 |
|---|---|
| 国内（推荐） | [Gitee Releases](https://gitee.com/kz_y/naxone/releases/latest) |
| 海外 | [GitHub Releases](https://github.com/OverfireWater/naxone/releases/latest) |

要求 Windows 10 1809+ / Windows 11，x64。

<details>
<summary>首次运行 Windows SmartScreen 警告怎么办？</summary>

NaxOne 没有付费代码签名证书，Windows 会弹"未识别的应用"。三选一绕过：

1. 弹窗里点 **更多信息** → **仍要运行**
2. 右键 .exe → **属性** → 勾选 **解除阻止** → 确定
3. PowerShell：`Unblock-File -Path "D:\NaxOne\NaxOne.exe"`

只是首次需要，之后正常启动。
</details>

## 🖼 界面

<table>
<tr>
<td width="50%"><a href="docs/screenshots/01-dashboard.png"><img src="docs/screenshots/01-dashboard.png" alt="仪表板" /></a><br><b>仪表板</b>：一行启停 Nginx/Apache/MySQL/Redis，PHP 多版本统一管理，活动日志快览</td>
<td width="50%"><a href="docs/screenshots/03-vhosts-modal-basic.png"><img src="docs/screenshots/03-vhosts-modal-basic.png" alt="新建站点" /></a><br><b>新建站点</b>：基础 / 伪静态 / SSL 三 tab，模板一键装包，自动签 HTTPS</td>
</tr>
<tr>
<td width="50%"><a href="docs/screenshots/08-config-nginx.png"><img src="docs/screenshots/08-config-nginx.png" alt="服务配置" /></a><br><b>服务配置</b>：Nginx 19 项 / MySQL 25 项 / Redis 20 项 / PHP 34 项可视化</td>
<td width="50%"><a href="docs/screenshots/06-store.png"><img src="docs/screenshots/06-store.png" alt="软件商店" /></a><br><b>软件商店</b>：PHP/Nginx/MySQL 多版本下载，多镜像加速</td>
</tr>
</table>

> 完整 13 张截图见 [docs/screenshots/](docs/screenshots/)

## ✨ 主要功能

- 🚀 **服务管理** —— 一键启停所有服务；Nginx/Apache 互斥联动 PHP-CGI；端口探测 + 进程名校验，状态零假阳性
- 🌐 **虚拟主机** —— Nginx + Apache 配置双写；自动写 hosts；一键 mkcert HTTPS（浏览器直接绿锁）
- 📦 **站点模板** —— 新建 vhost 选 WordPress / Laravel / ThinkPHP / Webman / 空白，自动下载或 composer 装包
- 🐘 **PHP 多版本** —— 每个站点独立 PHP 版本；全局 CLI `php` 一键切；Composer/Node/MySQL 同样
- ⚙️ **可视化配置** —— Nginx/MySQL/Redis/PHP 上百项参数图形编辑，自动 `.bak` 备份
- 📋 **活动日志** —— 所有写操作归档，关键字搜索、分类过滤、失败行高亮、完整 stdout 可回查
- 🏪 **软件商店** —— PHP 官方源 + GitHub 镜像，按需下载历史版本，SHA-256 校验
- 🔍 **端口诊断** —— 自动识别外部占用 80/3306/6379 的进程（含 PHPStudy 服务），一键结束

<details>
<summary>展开看更多细节</summary>

- **mkcert SSL**：自动建本地 CA 并装到当前用户证书库（不需要管理员权限），浏览器信任，无需手动导入
- **PHP 扩展**：基于 [PIE](https://github.com/php/pie) 一键装/卸；自动选 PHP 8.1+ 当 runtime
- **数值字段防呆**：服务配置保存前校验 `gzip_level (1-9)` 等范围，避免 nginx reload 报 emerg
- **Webman 模板**：自动配 `proxy_pass http://127.0.0.1:8787`，提示用户 `windows.bat` 启动 cli
- **Tauri auto-updater**：内置自动更新检查，新版本通过签名验证后无感升级
- **现代体验**：亮色/暗色主题切换、紧凑界面、无原生标题栏、系统托盘最小化

</details>

## 🤝 兼容性

| 场景 | 说明 |
|---|---|
| **PHPStudy Pro** | 自动扫描其 `Extensions` 目录里的 PHP/Nginx/Apache/MySQL/Redis；生成的 vhost 配置格式与 PHPStudy 完全一致，可双向迁移 |
| **PHP 官方包** | 直接识别 windows.php.net 下载的 zip 解压目录 |
| **配置文件** | 所有写操作前自动 `.bak` 备份 |
| **跨电脑** | 数据全部在 `%USERPROFILE%\.naxone\`，单文件夹拷贝即可迁移 |

## 🛠 从源码构建

需要 [Rust](https://rustup.rs/) >= 1.75 和 [Node.js](https://nodejs.org/) >= 20。

```bash
# 安装前端依赖
cd crates/naxone-tauri/frontend && npm install && cd ..

# 开发模式（Vite + Rust + Tauri 一条龙）
cargo tauri dev

# 打包 NSIS 安装包（输出在 target/release/bundle/nsis/）
cargo tauri build

# 运行测试
cargo test --workspace
```

## 🏗 架构

采用**六边形架构**（Hexagonal / Ports & Adapters），核心业务逻辑跟外部依赖完全解耦：

- **`naxone-core`** —— 纯领域逻辑，零外部依赖，可独立单元测试
- **`naxone-adapters`** —— 实现 core 定义的 Port traits（进程管理、文件 IO、模板渲染、平台 API），未来扩展到其他平台只换这层
- **`naxone-tauri`** —— 桌面 App 壳（Tauri IPC 转发 + 应用初始化 + Vue 前端）

![整体架构](docs/diagrams/architecture.png)

<details>
<summary>展开看完整项目结构</summary>

```
naxone/
├── Cargo.toml                          # Workspace 根
├── LICENSE                             # MIT
├── crates/
│   ├── naxone-core/                    # 纯领域逻辑
│   │   └── src/
│   │       ├── domain/                 # 领域模型：Service / VirtualHost / PHP / Log
│   │       ├── ports/                  # 端口 trait：ProcessManager / ConfigIO / TemplateEngine / PlatformOps
│   │       ├── use_cases/              # 用例：ServiceManager / VhostManager / PhpManager / ConfigEditor
│   │       ├── config.rs               # AppConfig
│   │       └── error.rs                # 统一错误类型
│   │
│   ├── naxone-adapters/                # 端口的具体实现
│   │   └── src/
│   │       ├── config/                 # FsConfigIO
│   │       ├── package/                # 包扫描 + 软件商店
│   │       ├── platform/               # WindowsPlatform（hosts、SSL 自签、全局 PHP shim）
│   │       ├── process/                # NativeProcessManager
│   │       ├── template/               # SimpleTemplateEngine
│   │       └── vhost/                  # VhostScanner
│   │
│   └── naxone-tauri/                   # 桌面 App 壳
│       ├── src/                        # Tauri 入口 + IPC 命令
│       ├── frontend/                   # Vue 3 + Vite + Tailwind
│       │   └── src/views/              # Dashboard / Vhosts / ServiceConfig / GlobalEnv / Settings
│       ├── tauri.conf.json
│       ├── nsis/installer-hooks.nsh    # NSIS 安装器自定义（默认装到 D:\NaxOne）
│       └── icons/                      # 应用图标
```

</details>

<details>
<summary>展开看服务启动 / 虚拟主机流程图</summary>

### 服务启动流程（含互斥 + 联动）
![服务启动流程](docs/diagrams/service-flow.png)

### 虚拟主机双写流程（Nginx + Apache 同步）
![虚拟主机双写流程](docs/diagrams/vhost-flow.png)

</details>

## 📜 许可证

[MIT](LICENSE) © 2026 NaxOne Contributors

被管理的二进制（PHP / Nginx / Apache / MySQL / Redis）各自遵循其原有许可证（PHP License / BSD / Apache 2.0 / GPL / BSD），NaxOne 仅作为本地启停和配置工具，**不**重分发上述二进制。
