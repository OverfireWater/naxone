<div align="center">

<img src="logo_transparent.png" alt="NaxOne" width="160" />

# NaxOne

**一站式本地开发集成环境** · 用 Rust 重写、Tauri 打包、Vue 现代界面

[![License: MIT](https://img.shields.io/github/license/OverfireWater/naxone?color=blue)](LICENSE)
[![GitHub release](https://img.shields.io/github/v/release/OverfireWater/naxone?label=release&color=brightgreen)](https://github.com/OverfireWater/naxone/releases)
[![GitHub downloads](https://img.shields.io/github/downloads/OverfireWater/naxone/total?label=downloads&color=orange)](https://github.com/OverfireWater/naxone/releases)
[![GitHub stars](https://img.shields.io/github/stars/OverfireWater/naxone?style=flat&label=GitHub%20stars)](https://github.com/OverfireWater/naxone/stargazers)
[![Gitee stars](https://gitee.com/kz_y/naxone/badge/star.svg?theme=dark)](https://gitee.com/kz_y/naxone/stargazers)
[![Gitee forks](https://gitee.com/kz_y/naxone/badge/fork.svg?theme=dark)](https://gitee.com/kz_y/naxone/members)

[下载安装包 (Gitee)](https://gitee.com/kz_y/naxone/releases/latest) · [下载安装包 (GitHub)](https://github.com/OverfireWater/naxone/releases/latest) · [问题反馈](https://gitee.com/kz_y/naxone/issues)

**中文** · [English](README_EN.md)

</div>

---

NaxOne 是面向 PHP 开发者的 Windows 本地开发集成环境管理器。一个原生桌面 App 把 Nginx / Apache / MySQL / Redis / 多版本 PHP 全部装进同一个面板，启停、配置、虚拟主机、SSL 一站搞定。

技术栈：**Rust + Tauri 2 + Vue 3 + TypeScript**。冷启动 < 1s，内存常驻 < 100MB，安装包仅 4MB。

> **跟 PHPStudy 是什么关系？** NaxOne **不依赖 PHPStudy**，可以独立安装、独立运行。但如果你机器上**已经有 PHPStudy**，NaxOne 会自动识别它的安装目录、PHP/Nginx/MySQL 包，把它们也纳入管理——无需重装、不破坏现有站点。两者互不影响、平滑共存。

## 功能亮点

- **服务管理**：一键启停 Nginx / Apache / MySQL / Redis / PHP-CGI；Nginx 与 Apache 自动互斥；启动 Web 服务器联动拉起 PHP-CGI；端口探测 + 进程名校验，状态绝不假阳性
- **虚拟主机**：创建/编辑/删除站点；Nginx + Apache 配置**双写**（切引擎零成本）；自动写入 hosts；改完即时 reload；伪静态预设（Laravel/ThinkPHP/WordPress）；一键自签 SSL
- **PHP 多版本**：本地装多个 PHP，每个站点选独立版本；全局 CLI `php` 命令一键切版本（在用户 PATH 下挂 shim，新开终端立即生效）
- **服务配置**：Nginx 19 项 / MySQL 25 项 / Redis 20 项 / PHP 34 项可视化配置；PHP 扩展开关；改动前自动 `.bak`
- **软件商店**：内置 PHP 官方源 + GitHub 镜像源，按需下载历史版本，多镜像加速
- **陌生进程检测**：仪表板自动识别外部占用 80/3306/6379 等端口的进程（含 PHPStudy 自带服务），点一下即可结束
- **现代体验**：紧凑暗色界面、无原生标题栏、系统托盘最小化、自动更新检查、内存监控

## 界面预览

<table>
<tr>
<td width="50%"><b>仪表板</b><br/>毛玻璃 + 多色光晕，Nginx/Apache/MySQL/Redis 一行启停，PHP 多版本统一管理<br/><img src="docs/screenshots/01-dashboard.png" alt="Dashboard" /></td>
<td width="50%"><b>新建站点</b><br/>三 tab：基础配置 / 伪静态预设 / SSL 与高级<br/><img src="docs/screenshots/03-vhosts-modal-basic.png" alt="新建站点" /></td>
</tr>
<tr>
<td><b>软件商店</b><br/>内置 PHP/Nginx/MySQL 等多版本下载，多镜像加速，sha256 校验<br/><img src="docs/screenshots/06-store.png" alt="软件商店" /></td>
<td><b>服务配置 · Nginx</b><br/>19 项常用 Nginx 选项可视化编辑，改动前自动 .bak 备份<br/><img src="docs/screenshots/08-config-nginx.png" alt="Nginx 配置" /></td>
</tr>
<tr>
<td><b>全局环境</b><br/>CLI <code>php</code>/<code>composer</code>/<code>node</code>/<code>mysql</code> 一键切版本<br/><img src="docs/screenshots/07-config-env.png" alt="全局环境" /></td>
<td><b>设置</b><br/>PHPStudy 路径 / WWW 根目录 / 端口 / 自启动 / 主题<br/><img src="docs/screenshots/13-settings.png" alt="设置" /></td>
</tr>
</table>

> 更多截图见 [docs/screenshots/](docs/screenshots/)（13 张，覆盖所有页面）

## 安装

直接下最新版安装包：

| | 链接 |
|---|---|
| 国内（推荐） | [Gitee Releases](https://gitee.com/kz_y/naxone/releases/latest) |
| 海外 | [GitHub Releases](https://github.com/OverfireWater/naxone/releases/latest) |

文件名 `NaxOne_X.Y.Z_x64-setup.exe`，约 4 MB。NSIS 打包，**默认装到 `D:\NaxOne`**（如果你 D 盘存在，否则 `C:\NaxOne`）。

要求 Windows 10 1809+ / Windows 11，x64 架构。

### 首次运行：Windows SmartScreen 警告

**这是正常的**。NaxOne 当前未购买商业代码签名证书（年费 ¥1000+），首次运行时 Windows 会弹：

> Windows 已保护你的电脑
> Microsoft Defender SmartScreen 阻止了启动一个未识别的应用…

**绕过方法**（任选其一）：

1. **从安装器**：弹窗里点 **更多信息** → **仍要运行**
2. **从 .exe 文件**：右键文件 → **属性** → 底部勾选 **解除阻止** → 确定，然后正常双击运行
3. **PowerShell**：`Unblock-File -Path "D:\NaxOne\NaxOne.exe"`

**为什么不签名？** 自签证书 SmartScreen 仍会拦，OV 商业证书 ¥1000+/年仍会拦（要积累下载量"养信誉"），EV 证书 ¥3000+/年才能立即被信任。开源项目暂未投入。源码完全公开，可自审 + 自构建。

未来计划申请 [SignPath Foundation](https://signpath.org/) 的开源免费签名服务（Stellarium / Flameshot / GitExtensions 都在用），等项目获得一定关注后启动。

## 从源码构建

### 前置

- [Rust](https://rustup.rs/) >= 1.75
- [Node.js](https://nodejs.org/) >= 20

### 开发模式

```bash
# 安装前端依赖
cd crates/naxone-tauri/frontend
npm install

# 启动（自动跑 Vite + 编译 Rust + 起 Tauri 窗口）
cd ..
cargo tauri dev
```

### 打包

```bash
cargo tauri build
# 安装包位于 target/release/bundle/nsis/
```

### 运行测试

```bash
cargo test --workspace
```

## 架构

采用**六边形架构**（Hexagonal / Ports & Adapters），核心业务逻辑跟外部依赖完全解耦。

```mermaid
flowchart TB
    UI["🖥️ 前端 · Vue 3 + Vite + Tailwind<br/>views/ · components/ · 自定义标题栏"]
    UI -->|"Tauri invoke()"| TAURI

    subgraph TAURI ["📦 naxone-tauri · IPC 桥 + 应用状态"]
      direction LR
      CMD["commands/<br/>service · vhost · php · settings · package · updater"]
      STATE["state.rs<br/>AppState · 配置加载 · 老用户迁移"]
    end

    TAURI --> CORE
    subgraph CORE ["🔷 naxone-core · 域 + 用例（零外部依赖）"]
      direction LR
      DOMAIN["domain/<br/>Service · VirtualHost · PHP · Log"]
      USECASE["use_cases/<br/>ServiceManager · VhostManager · PhpManager"]
      PORTS["ports/<br/>ProcessManager · ConfigIO · TemplateEngine · PlatformOps"]
    end

    CORE -.implements.-> ADAPTERS
    subgraph ADAPTERS ["🔧 naxone-adapters · 端口实现（平台相关）"]
      direction LR
      PROC["NativeProcessManager<br/>启停 · 端口探测 · PID 跟踪"]
      CFG["FsConfigIO<br/>原子写 · .bak 备份"]
      TPL["SimpleTemplateEngine<br/>nginx/apache vhost 渲染 + 注入校验"]
      PLAT["WindowsPlatform / LinuxPlatform<br/>hosts · SSL 自签 · 全局 PHP shim"]
      SCAN["PackageScanner<br/>PHPStudy 扫描 + 软件商店"]
    end

    ADAPTERS --> WIN["🪟 Windows API<br/>WMI · netsh · icacls · UAC 提权"]
    ADAPTERS --> NET["🌐 reqwest<br/>包下载 + sha256 校验"]
```

- **`naxone-core`**：纯领域逻辑，零外部依赖，可独立单元测试
- **`naxone-adapters`**：实现 core 定义的 Port traits，可替换（未来做 Linux TUI 只换这层）
- **`naxone-tauri`**：只做 IPC 转发 + 应用初始化

### 服务启动流程（含互斥 + 联动）

```mermaid
sequenceDiagram
    participant User as 用户
    participant UI as 前端
    participant CMD as Tauri Command
    participant SM as ServiceManager
    participant PM as ProcessManager
    participant FS as 文件系统

    User->>UI: 点击"启动 Nginx"
    UI->>CMD: invoke("start_service", id)
    CMD->>SM: start_with_deps(target, others)

    Note over SM: 1. 互斥处理
    SM->>PM: 查 Apache 是否在跑
    alt Apache 在跑
        SM->>PM: stop(Apache)
    end

    Note over SM: 2. 启动 target
    SM->>PM: spawn(nginx.exe)
    PM->>FS: 读 nginx.conf + 校验
    PM-->>SM: PID + 状态

    Note over SM: 3. 联动 PHP-CGI
    loop 每个未启动的 PHP 实例
        SM->>PM: spawn(php-cgi.exe -b :port)
        PM-->>SM: PID
    end

    SM-->>CMD: services 数组（含所有变更）
    CMD-->>UI: emit "services-changed"
    UI->>User: Toast "已启动 Nginx + 3 个 PHP-CGI"
```

### 虚拟主机双写流程（Nginx + Apache 同步）

```mermaid
flowchart LR
    A["用户提交表单"] --> B{"vhost.validate()"}
    B -->|"含 ; 或 路径穿越"| Z1["❌ 拒绝<br/>注入防护"]
    B -->|"通过"| C{"validate_document_root"}
    C -->|"指向 C:\Windows"| Z2["❌ 拒绝<br/>系统目录保护"]
    C -->|"通过"| D["build_vhost"]

    D --> E["写 Nginx vhost conf<br/>(原子 tempfile + rename)"]
    E -->|"失败"| R1["回滚"]
    E -->|"成功"| F["写 Apache vhost conf"]
    F -->|"失败"| R2["回滚 Nginx"]
    F -->|"成功"| G["更新 hosts 文件"]
    G --> H{"Web 服务器在跑?"}
    H -->|"是"| I["nginx -t + reload"]
    I -->|"配置错"| R3["全部回滚"]
    I -->|"OK"| J["✅ 完成"]
    H -->|"否"| J
```

## 项目结构

```
naxone/
├── Cargo.toml                          # Workspace 根（统一版本、共享依赖）
├── LICENSE                             # MIT
├── logo.png / logo_transparent.png     # 品牌资源
├── crates/
│   ├── naxone-core/                    # 纯领域逻辑，零外部依赖
│   │   └── src/
│   │       ├── domain/                 # 领域模型：Service / VirtualHost / PHP / Log
│   │       ├── ports/                  # 端口 trait：ProcessManager / ConfigIO / TemplateEngine / PlatformOps
│   │       ├── use_cases/              # 用例：ServiceManager / VhostManager / PhpManager / ConfigEditor
│   │       ├── config.rs               # AppConfig（TOML 配置反序列化）
│   │       └── error.rs                # 统一错误类型
│   │
│   ├── naxone-adapters/                # 端口的具体实现
│   │   └── src/
│   │       ├── config/                 # FsConfigIO（文件 IO）
│   │       ├── package/                # 包扫描 + 软件商店（PHP 官方源 / GitHub 镜像源）
│   │       ├── platform/               # WindowsPlatform / LinuxPlatform（hosts、SSL 自签、全局 PHP shim）
│   │       ├── process/                # NativeProcessManager（进程启停 + 端口探测）
│   │       ├── template/               # SimpleTemplateEngine（生成 nginx/apache vhost 配置）
│   │       └── vhost/                  # VhostScanner（解析现有虚拟主机）
│   │
│   └── naxone-tauri/                   # 桌面 App 壳
│       ├── src/
│       │   ├── main.rs                 # Tauri 入口（系统托盘、插件注册）
│       │   ├── state.rs                # AppState（依赖注入、配置加载、老用户迁移）
│       │   └── commands/               # Tauri IPC 命令：service / vhost / php / settings / package / updater ...
│       ├── frontend/                   # Vue 3 + Vite + Tailwind 前端
│       │   └── src/
│       │       ├── App.vue             # 根布局（自定义标题栏、侧栏、路由）
│       │       ├── views/              # 页面：Dashboard / Vhosts / ServiceConfig / Settings
│       │       ├── components/         # 复用组件：StoreCard / LogDrawer / SelectMenu ...
│       │       └── assets/             # global.css（Tailwind + 主题变量）
│       ├── tauri.conf.json             # Tauri 配置
│       ├── nsis/installer-hooks.nsh    # Windows 安装器自定义（默认装到 D:\NaxOne）
│       ├── icons/                      # 应用图标（自动从 logo.png 生成）
│       └── capabilities/               # Tauri 权限配置
```

## 兼容性

- **PHPStudy Pro**：自动扫描其 `Extensions` 目录里的 PHP/Nginx/Apache/MySQL/Redis 包；生成的 vhost 配置格式与 PHPStudy 完全一致，可双向迁移
- **PHP 官方包**：直接识别 windows.php.net 下载的 zip 解压目录
- **配置文件**：所有写操作前自动 `.bak` 备份
- **从 RustStudy 升级**：首次启动 NaxOne 会自动迁移 `~/.ruststudy` 和 `%APPDATA%\RustStudy` 到对应的 NaxOne 目录

## 许可证

[MIT](LICENSE) © 2026 NaxOne Contributors

PHP/Nginx/Apache/MySQL/Redis 等被管理的二进制各自遵循其原有许可证（PHP License / BSD / Apache 2.0 / GPL / BSD），NaxOne 仅作为本地启停和配置工具，不重分发上述二进制。
