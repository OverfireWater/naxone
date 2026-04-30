## NaxOne v0.3.0

### 改名

项目从 **RustStudy** 更名为 **NaxOne**。

- **Nax** 取自 Navigator / Next，意指引领与连接
- **One** 表示一站式、一体化
- 整体定位：**本地开发集成环境**

更名原因：原名 `RustStudy` 与商业产品 `phpstudy / 小皮` 在后缀上构成商标近似风险。改名是为了在未来推广和长期维护中规避法律隐患，与 phpstudy 划清边界。

### 老用户自动迁移

从 RustStudy v0.2.x 升级，**首次启动 NaxOne 0.3.0 会自动完成数据迁移**，无需任何手动操作：

- `~/.ruststudy/` → `~/.naxone/`（含 vhosts.json、SSL 证书、缓存等所有子目录）
- `~/.ruststudy/ruststudy.toml` → `~/.naxone/naxone.toml`（自动改名）
- `%APPDATA%\RustStudy\` → `%APPDATA%\NaxOne\`（packages 安装树）

**迁移逻辑是幂等的**：只在新目录不存在、且老目录存在时执行一次。迁移失败不影响主流程，会在日志中记录 warn。

老的 `~/.ruststudy/` 目录迁移后**不会自动删除**，确认 NaxOne 一切正常后你可以手动删除。

### 升级建议

1. 卸载 RustStudy 0.2.x **之前**，请确认 `%USERPROFILE%\.ruststudy\` 目录还在（卸载器不会动它，但保险起见自己留个心）
2. 安装 NaxOne 0.3.0
3. 启动 NaxOne，配置应该已经全部继承过来——站点列表、SSL 证书、PHP 版本、Service 配置都在
4. 确认无误后可以手动删除 `~/.ruststudy/` 和 `%APPDATA%\RustStudy\`

### 其他变化

- 全新 Logo（蓝/青渐变 N 字）
- 应用安装目录默认 `D:\NaxOne` 或 `C:\NaxOne`（之前是 `D:\RustStudy`）
- HTTP user-agent 从 `RustStudy/x.x.x` 改为 `NaxOne/x.x.x`
- 主题/语言等浏览器 localStorage 偏好会**重置**（key 从 `ruststudy-theme` 改为 `naxone-theme`），首次启动需要重新选一次主题
