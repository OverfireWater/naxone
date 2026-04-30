# NaxOne

> 用 Rust 重写的 PHPStudy —— 轻量、快速、现代化的本地 PHP 开发环境管理器

NaxOne 是一个 Windows 桌面应用，用于替代 PHPStudy Pro，管理本地开发环境中的 Nginx / Apache / MySQL / Redis / PHP 服务。兼容现有 PHPStudy Pro 的安装目录和配置格式。

## 功能

### 服务管理
- 一键启动 / 停止 / 重启 Nginx、Apache、MySQL、Redis、PHP-CGI
- Nginx 和 Apache 互斥（自动停止另一个）
- 启动 Web 服务器时自动联动启动 PHP-CGI
- 实时进程状态检测（通过端口探测 + 进程名验证）

### 虚拟主机管理
- 创建 / 编辑 / 删除虚拟主机
- Nginx 和 Apache 配置**双写**（切换 Web 服务器零成本）
- 自动更新 Windows hosts 文件
- 修改后自动 reload Nginx / Apache
- 伪静态预设（Laravel / ThinkPHP / WordPress）
- SSL / HTTPS 支持
- 自定义 Nginx 指令

### 服务配置
- **Nginx**：工作进程、连接数、超时、Gzip、请求限制等 19 项配置
- **MySQL**：连接数、InnoDB 缓冲池、字符集、超时、缓冲区等 25 项配置
- **Redis**：端口、内存、持久化、密码、淘汰策略等 20 项配置
- **PHP**：扩展开关（toggle）、php.ini 配置（基础 / 安全 / OPCache / Session 等 34 项）

### 全局设置
- PHPStudy 路径 / WWW 根目录
- Web 服务器切换（Nginx / Apache）
- 端口配置（MySQL / Redis）
- 自动启动服务选择
- 暗色 / 亮色 / 跟随系统主题切换

### 系统托盘
- 关闭窗口最小化到托盘（不退出）
- 双击托盘图标恢复窗口
- 右键菜单：显示主窗口 / 退出

## 技术栈

| 层 | 技术 |
|----|------|
| 后端 | Rust + Tokio + Tauri v2 |
| 前端 | Vue 3 + TypeScript + Vite + Tailwind CSS 3 |
| 图标 | Lucide Icons |
| 架构 | 六边形架构（Hexagonal / Ports & Adapters） |

## 项目结构

```
naxone/
├── Cargo.toml                    # Workspace 根配置
├── crates/
│   ├── naxone-core/           # 核心域（平台无关）
│   │   └── src/
│   │       ├── domain/           # 领域模型（Service, VirtualHost, PHP）
│   │       ├── ports/            # 端口 Traits（ProcessManager, ConfigIO, TemplateEngine, PlatformOps）
│   │       ├── use_cases/        # 用例（ServiceManager, VhostManager, PhpManager, ConfigEditor）
│   │       ├── config.rs         # AppConfig（TOML 配置）
│   │       └── error.rs          # 统一错误类型
│   │
│   ├── naxone-adapters/       # 适配器（具体实现）
│   │   └── src/
│   │       ├── config/           # FsConfigIO（文件系统读写）
│   │       ├── package/          # PhpStudyScanner（扫描 Extensions 目录）
│   │       ├── platform/         # WindowsPlatform / LinuxPlatform（hosts 文件管理）
│   │       ├── process/          # WindowsProcessManager（进程启停）
│   │       ├── template/         # SimpleTemplateEngine（Nginx/Apache 配置生成）
│   │       └── vhost/            # VhostScanner（解析现有虚拟主机）
│   │
│   └── naxone-tauri/          # Tauri 应用壳
│       ├── src/
│       │   ├── main.rs           # Tauri 入口（系统托盘、插件注册）
│       │   ├── state.rs          # AppState（依赖注入、初始化）
│       │   └── commands/         # IPC Commands（service, vhost, php, settings, service_config）
│       ├── frontend/             # Vue 3 前端
│       │   └── src/
│       │       ├── App.vue       # 根布局（标题栏、侧栏、路由）
│       │       ├── assets/       # global.css（Tailwind + 主题变量）
│       │       └── views/        # 页面（Dashboard, Vhosts, ServiceConfig, Settings）
│       ├── tauri.conf.json       # Tauri 配置
│       ├── capabilities/         # 权限配置
│       └── icons/                # 应用图标
```

## 开发

### 前置条件

- [Rust](https://rustup.rs/) >= 1.75
- [Node.js](https://nodejs.org/) >= 20
- [PHPStudy Pro](https://www.xp.cn/) 已安装（默认路径 `D:\phpstudy_pro`）

### 运行

```bash
# 克隆项目
cd D:\phpstudy_pro\WWW\utils\naxone

# 安装前端依赖
cd crates/naxone-tauri/frontend
npm install

# 启动开发模式（自动编译 Rust + 启动 Vite）
cd ..
cargo tauri dev
```

### 构建

```bash
cargo tauri build
```

生成的安装包在 `target/release/bundle/` 目录。

### 发布到 Gitee Release

```bash
# 先准备 token（不要提交到仓库）
export GITEE_TOKEN=...

# 或者先 source 本地忽略文件
source release.env.local

# 构建后上传当前版本的安装包
python scripts/release_gitee.py
```

### 运行测试

```bash
cargo test -p naxone-adapters -- --nocapture
```

## 架构说明

采用**六边形架构**（Hexagonal Architecture），核心业务逻辑与外部依赖完全解耦：

```
Frontend (Vue 3)
    ↓ invoke()
Tauri Commands (IPC 桥接)
    ↓
Use Cases (业务编排)
    ↓
Ports (Trait 接口)
    ↓
Adapters (具体实现: 文件系统 / 进程管理 / 模板引擎)
```

- **Core** 不依赖任何外部 crate（纯 Rust + serde），可独立测试
- **Adapters** 实现 Core 定义的 Trait，可替换（如未来做 Linux TUI）
- **Tauri** 层只做 IPC 转发和状态管理

## 兼容性

- 兼容 PHPStudy Pro 的 `Extensions` 目录结构
- 自动扫描已安装的 PHP / Nginx / Apache / MySQL / Redis 版本
- 生成的虚拟主机配置格式与 PHPStudy 完全一致
- 配置文件修改前自动备份（`.bak`）

## 许可证

MIT
