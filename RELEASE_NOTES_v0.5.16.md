## NaxOne v0.5.16

本次版本累积了一次大规模的真实使用 bug 修复，覆盖**站点全生命周期**：从装 PHP/Nginx → 创建站点 → 装模板 → 浏览器访问，每一步的隐患都做了加固。强烈推荐升级。

### ✨ 新增

- **Composer 镜像源切换**：「全局环境」composer 卡片下新增镜像下拉（Packagist 官方 / 阿里云 / 腾讯云 / 华为云 / 自定义 URL），直接读写 `%APPDATA%/Composer/config.json`，告别 `composer config` 命令对数组形式 repositories 字段的兼容性问题
- **网站页面新装引导**：未检测到 Nginx / Apache 时顶部显示橙色引导卡片 + "+ 新建站点"按钮置灰，引导用户去软件商店一键安装
- **后台模板装包**：装 Laravel / ThinkPHP / Webman 等模板时 modal 可关闭，下载继续在后台运行，完成 / 失败弹 toast 通知

### 🔧 关键 Bug 修复

- **nginx.conf 自动 include vhosts/**：商店装的官方 nginx 默认 conf 不含 `include vhosts/*.conf;`，导致 NaxOne 写的 vhost 配置无法加载（浏览器看到 nginx 欢迎页）。现在 create_vhost 自动注入这一行（原文件 `.bak` 备份）
- **卸载 nginx 不再丢站点配置**：之前卸载 nginx 会把 install_dir 整目录删掉，conf/vhosts/*.conf 全部丢失。现在：卸载前弹窗显示"将删除 N 个站点配置"；重装后 NaxOne 自动从 `~/.naxone/vhosts.json` 元数据 regenerate 所有缺失的 .conf；站点源码、hosts、SSL 证书全程不丢
- **商店 PHP 默认 ini 修复**：商店装的 PHP 包默认 `php.ini-production` 的 extension_dir 行全是注释，导致 openssl / mbstring 等扩展全部加载失败 → composer create-project 跑不动、PHP-CGI 启动报警告。现在装 PHP 时自动 copy `php.ini-production → php.ini` 并解注释 `extension_dir = "ext"`；启动时遍历历史装的 PHP 一次性修好
- **解除关联现在真的生效**：之前解除系统装的 Composer 关联无效（PATH 删的是 HKCU，但 ComposerSetup 装在 HKLM）。重新设计语义：解除关联 = NaxOne 不再视野管理，写入 `config.ignored_system_tools`，**不动**用户的系统 PATH / 文件，避免影响 IDE 等其他依赖
- **GlobalEnv 切换全局后 Dashboard 立即刷新**：切 PHP / Composer / Node / MySQL 后通过 Tauri event 通知 Dashboard 刷新环境概览，不再需要重启
- **切换 PHPStudy 路径清旧 vhost**：保存设置时若 phpstudy_path 变化，自动清理 vhosts.json 里 source=PhpStudy 的旧元数据（Custom 自建站点保留），前端 vhost 列表立即刷新
- **新建 vhost 选错 PHP 不再静默坏**：如果选了一个 services 里不存在的 PHP 版本（PHPStudy 路径配错、PHP 已卸载等），直接报错让用户去解决而不是生成无 fastcgi 块的破 vhost

### 🎨 UX 改善

- **站点模板装包绕开系统 ComposerSetup**：强制使用 NaxOne 内置 PHP + composer.phar，避免 PATH 优先级里命中系统装的 composer 走错 PHP 配置
- **PHP 调用强制 `-c <NaxOne php.ini>`**：避免读到 `C:\php\php.ini`（用户机器装独立 PHP 时常发生）
- **软件商店统一弹窗风格**：替换/卸载/解除关联/彻底卸载 4 处原生 confirm 全改为 ConfirmDialog 组件，与项目整体玻璃毛风格一致
- **卸载 toast 文案区分**：原本统一说"已卸载"，现在区分"已卸载 / 已解除关联 / 已彻底卸载"，且版本号为 `?` 时省略 `v?`
- **消除子进程 cmd 黑框闪烁**：taskkill / netsh / powershell 等 4 处 Command 补 CREATE_NO_WINDOW
- **防火墙规则失败静默处理**：非管理员模式下 netsh 失败不再 spam Warn（本地访问 vhost 走 loopback 不经防火墙，没必要打扰用户）
- **www_root 残留 `D:\RustStudy` 自动迁移**：从 RustStudy 改名 NaxOne 后，老用户配置里的 www_root 启动时自动迁到 `<exe>/www`

### 升级方式

已在用 NaxOne 的，下次启动会自动收到更新提示。
新装请下载附件 `NaxOne_0.5.16_x64-setup.exe` 双击安装。

升级后首次启动会自动跑两个迁移任务：
1. 给所有已装的商店 PHP 修 php.ini 的 extension_dir（幂等）
2. 从 vhosts.json 重新生成所有缺失的 nginx vhost .conf（幂等）

两个任务都在后台异步执行，对启动速度影响 < 100ms。
