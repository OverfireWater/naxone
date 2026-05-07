# NaxOne v0.5.6

## 关键修复

- **修复版本检查 ACL 报错**：`capabilities/default.json` 加 `core:app:default` 权限。v0.5.5 启动时弹出的 `Command plugin:app|version not allowed by ACL` 提示已彻底解决，更新 banner 重新可见。
- **加固更新检查代码**：`getVersion()` 拆出独立 try，未来即使再出现单点权限漏配也不会让整块更新检查失败。

## 新功能

- **设置页新增「应用更新」面板**：
  - 左侧显示当前版本号
  - 中间是圆形蓝色刷新按钮，点击有旋转动画
  - 右侧显示状态（"点击检查"/"已是最新"/红点+"有新版本 vX.Y.Z 点击更新"）
  - 显示上次检查的完整时间（含日期）
  - disabled 时按钮变灰、禁用悬浮缩放，视觉反馈更准确

## 升级方式

- v0.5.5 用户：手动下载本页的 `NaxOne_0.5.6_x64-setup.exe` 覆盖安装一次（因为 ACL 是编译进 exe 的，旧版本无法 OTA 自检）
- v0.5.6 起：可以在「设置 → 应用更新」点圆形按钮检查，或保持仪表盘顶部 banner 提示
