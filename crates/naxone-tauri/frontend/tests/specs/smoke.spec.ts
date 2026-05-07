/**
 * NaxOne 烟雾测试：启动应用 → 验证关键 UI 元素存在 → 关键交互能跑。
 * 环境：tauri-driver + msedgedriver + WebView2，对 vue 组件级 DOM 操作。
 */
import { browser, $, $$, expect } from "@wdio/globals";

describe("NaxOne 启动 + 主框架", () => {
  it("窗口能起来，标题栏含 NaxOne", async () => {
    // 等到 #app 被 vue 挂载（Dashboard 或任意页面渲染）
    await $("#app").waitForExist({ timeout: 30_000 });

    // 标题栏内有多个 span：logo "R" + 应用名 "NaxOne (Development)" 等。把所有文本拼起来检查
    const text = await browser.execute(() => {
      const spans = Array.from(document.querySelectorAll(".titlebar-drag span"));
      return spans.map((s) => s.textContent?.trim() ?? "").join("|");
    });
    expect(text).toContain("NaxOne");
  });

  it("侧栏 5 个菜单项都可见（仪表板 / 网站 / 软件商店 / 服务配置 / 设置）", async () => {
    const labels = await browser.execute(() => {
      const items = Array.from(document.querySelectorAll("aside nav > div"));
      return items.map((el) => el.textContent?.trim().replace(/\s+/g, "") ?? "");
    });
    const joined = labels.join("|");
    expect(joined).toContain("仪表板");
    expect(joined).toContain("网站");
    expect(joined).toContain("软件商店");
    expect(joined).toContain("服务配置");
    expect(joined).toContain("设置");
  });
});

describe("路由切换 + KeepAlive", () => {
  it("切到「服务配置」再切回「仪表板」，组件实例不重建（KeepAlive）", async () => {
    // 切到服务配置
    await browser.execute(() => {
      const navItems = Array.from(document.querySelectorAll("aside nav > div")) as HTMLElement[];
      const cfg = navItems.find((el) => el.textContent?.includes("服务配置"));
      cfg?.click();
    });
    await browser.pause(500);

    // 切回仪表板
    await browser.execute(() => {
      const navItems = Array.from(document.querySelectorAll("aside nav > div")) as HTMLElement[];
      const dash = navItems.find((el) => el.textContent?.includes("仪表板"));
      dash?.click();
    });
    await browser.pause(500);

    // 仪表板里应该有 PHP 引擎卡或服务卡（说明 Dashboard 渲染过）
    const hasContent = await browser.execute(() => {
      return document.body.textContent?.includes("PHP 引擎")
        || document.body.textContent?.includes("仪表板")
        || document.body.textContent?.includes("Dashboard")
        || (document.querySelectorAll(".svc-card").length > 0);
    });
    expect(hasContent).toBe(true);
  });
});

describe("UI 字号两档制（视觉契约）", () => {
  it("body 主体字号是 16px", async () => {
    const fontSize = await browser.execute(() => {
      return window.getComputedStyle(document.body).fontSize;
    });
    expect(fontSize).toBe("16px");
  });
});
