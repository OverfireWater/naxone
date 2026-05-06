## NaxOne v0.4.0

### 新增「全局环境」面板

「服务配置」页加入新 tab **全局环境**（默认进入这一栏），统一管理命令行 `php` / `composer` / `node` / `mysql` 走哪个版本，以及 MySQL root 密码。

- **PHP / Composer / Node**：原仪表盘的版本切换器整体迁移过来，UI 重新整理为卡片式
- **MySQL 多源管理**：自动检测**所有来源**的 MySQL 安装，每条带徽章
  - `[NX]` 商店安装
  - `[PS]` PHPStudy
  - `[独立]` 用户在「设置」里指定的额外安装路径
  - `[系统]` 自动从系统目录扫到的
- **活跃版本检测**：合并查询用户 PATH (HKCU) + 系统 PATH (HKLM)，PHPStudy 把 bin 写到系统 PATH 也能正确识别
- **系统 PATH 一键修复**：检测到 HKLM 里有屏蔽你"设为全局"的 MySQL 目录时显示警告条，点一下走 UAC 提权清理
- **root 密码**：商店装 MySQL 时 post-install 自动设默认密码 `root` 并写到 `<install>/.naxone-root.txt`；PHPStudy 等外来 MySQL 第一次改密码时会让你手动输入当前密码

### 商店安装 MySQL 自动初始化

之前商店下载完 MySQL 是裸目录，启动会因为没有 `data/` 失败。现在 post-install 会一气做完：

- 生成 `my.ini`（utf8mb4 / mysql_native_password / 默认端口 3306 等合理预设）
- `mysqld --initialize-insecure` 创建 data 目录与系统库
- 临时端口启 mysqld + `--init-file` 跑 `ALTER USER ... IDENTIFIED BY 'root'` → mysqladmin shutdown
- 写 `.naxone-root.txt`，仪表盘 / 全局环境页可以直接看到密码

### 仪表盘瘦身

之前仪表盘混着服务卡 + 4 块版本切换器（全局 PHP / Composer / Node / MySQL），信息密度过高。现在：

- 删除版本切换控件，只保留**只读环境摘要**（一行小灰条：`PHP v8.4.2 · Composer v2.9.7 · Node v20.x · MySQL v8.0.40 ›`）
- 点击摘要条直接跳到「服务配置 → 全局环境」tab
- 服务卡、PHP 引擎芯片、活动日志、陌生进程提醒等保持原样

### 商店卸载体验修复

- 卸载失败时错误信息原本被前端隐藏（错误状态被立刻重置），点击像没反应。现在错误条会内联显示在卡片下方
- 安装/卸载完成弹绿色 toast `XXX vY.Z.W 已安装 / 已卸载`，不再静默成功
- 修了一个误判 bug：当系统装的 MySQL（如 PHPStudy 自带的 5.7.26，由管理员启）占着 3306 端口、且 NaxOne 拿不到对方 exe 路径时，旧版会**默认认为是自己的实例在运行**，导致明明商店版没启动却报"v8.0 正在运行，请先停止"。现在拿不到 exe 路径时保守判定为不是本实例

### UI / 主题

- 服务配置页的 tab 顺序：**全局环境 | Nginx | MySQL | Redis | PHP | Hosts**
- URL 同步当前 tab（`/config?tab=mysql` 等），刷新或贴链接直接定位
- 暗/亮色模式新增 `--color-warn` 变量（暗:`#fbbf24` / 亮:`#b45309`），原硬编码 `#f59e0b` 在亮色下刺眼的问题修复
- PHP 扩展管理头部布局修复（之前下拉占满整行，扩展管理 / php.ini 配置两个 tab 头被挤成竖条）

### 升级建议

直接覆盖安装。已有的 `.naxone-root.txt`、用户 PATH、商店包都不会动。

- 如果你之前一直在 0.3.x 用 PHPStudy MySQL，第一次进「全局环境」会看到它出现在列表里——这是正常的
- 如果"设为全局"对 MySQL 不生效（系统 PATH 有别的 mysqld 屏蔽），按警告条提示一键修复
