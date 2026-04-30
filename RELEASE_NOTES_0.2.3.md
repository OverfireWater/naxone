## RustStudy v0.2.3

### 修复

- **www_root 自动迁移**：之前在 dev 模式（`cargo tauri dev`）下首次启动时，`www_root` 会被写成 `target\debug\www`，正式版后续启动只读不重算，导致"装到 D 盘但新建站点默认目录还在 cargo 编译产物里"。现在启动时会识别 `\target\debug\` 或 `\target\release\` 这种脏路径，自动重置为 `{exe 父目录}/www` 并落盘
- **版本号统一**：之前 user-agent 与侧边栏 v 标签是硬编码字符串（0.2.2 发布后侧边栏仍显示 v0.2.1）。现在 4 处版本展示全部从 `Cargo.toml` 编译期注入：
  - 三处 HTTP user-agent 改用 `env!("CARGO_PKG_VERSION")`
  - 侧边栏改用新增的 `get_app_version` Tauri 命令
  - 以后只需改 `Cargo.toml` 一处

### 升级建议

从 0.2.2 升级：直接覆盖安装即可，启动后会自动迁移配置文件里的脏 `www_root`。如果你之前手动改过 `www_root`，迁移逻辑只针对落在 `\target\debug\` 或 `\target\release\` 下的路径，不会动你自定义的设置。
