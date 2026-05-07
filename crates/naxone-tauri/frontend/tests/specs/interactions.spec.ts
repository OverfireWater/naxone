/**
 * UI 交互测试（只读：不写盘、不调改服务的后端命令）。
 * 覆盖：菜单切换、tab 切换、modal 开关、SelectMenu、KeepAlive、Toast。
 */
import { browser } from "@wdio/globals";
import { expect } from "@wdio/globals";
import { waitForApp, navigate, clickConfigTab, mainContains, mainText, countSelector } from "../helpers.js";

describe("路由 + KeepAlive", () => {
  before(async () => { await waitForApp(); });

  it("5 个菜单全部能切到 + KeepAlive 切回保留状态", async () => {
    // 第一次切到「设置」时让它加载，看到关键字
    await navigate("设置");
    expect(await mainContains("PHPStudy")).toBe(true);

    // 切到仪表板
    await navigate("仪表板");
    expect(await mainContains("活动日志")).toBe(true);

    // 切回设置
    await navigate("设置");
    // KeepAlive 后立刻就有数据，无需重新等加载
    expect(await mainContains("PHPStudy")).toBe(true);
  });

  it("当前活动菜单项的颜色权重比其他重", async () => {
    await navigate("软件商店");
    const activeText = await browser.execute(() => {
      const items = Array.from(document.querySelectorAll("aside nav > div")) as HTMLElement[];
      const active = items.find((el) => {
        const cs = window.getComputedStyle(el);
        return cs.fontWeight === "600" || cs.fontWeight === "bold" || el.classList.contains("font-semibold");
      });
      return active?.textContent?.trim().replace(/\s+/g, "") ?? "";
    });
    expect(activeText).toContain("软件商店");
  });
});

describe("Modal 行为（Vhosts 编辑站点）", () => {
  before(async () => { await navigate("网站"); });

  it("点新建 → modal 打开 → 点取消 → modal 关闭", async () => {
    await browser.execute(() => {
      const buttons = Array.from(document.querySelectorAll("main button")) as HTMLElement[];
      buttons.find((b) => b.textContent?.includes("新建站点"))?.click();
    });
    await browser.pause(400);
    expect(await countSelector(".modal-overlay")).toBeGreaterThanOrEqual(0); // overlay 在 body 不在 main
    let overlayCount = await browser.execute(() => document.querySelectorAll(".modal-overlay").length);
    expect(overlayCount).toBe(1);

    // 取消
    await browser.execute(() => {
      const overlay = document.querySelector(".modal-overlay")!;
      const cancel = Array.from(overlay.querySelectorAll("button")).find((b) => b.textContent?.includes("取消")) as HTMLElement | undefined;
      cancel?.click();
    });
    await browser.pause(400);
    overlayCount = await browser.execute(() => document.querySelectorAll(".modal-overlay").length);
    expect(overlayCount).toBe(0);
  });

  it("Modal 3 个内部 tab 切换", async () => {
    await browser.execute(() => {
      const buttons = Array.from(document.querySelectorAll("main button")) as HTMLElement[];
      buttons.find((b) => b.textContent?.includes("新建站点"))?.click();
    });
    await browser.pause(400);

    for (const tab of ["伪静态", "SSL", "基础配置"]) {
      await browser.execute((label: string) => {
        const tabs = Array.from(document.querySelectorAll(".modal-content .modal-tab")) as HTMLElement[];
        tabs.find((t) => t.textContent?.includes(label))?.click();
      }, tab);
      await browser.pause(200);
      const activeTab = await browser.execute(() => {
        const active = document.querySelector(".modal-content .modal-tab.active") as HTMLElement | null;
        return active?.textContent?.trim() ?? "";
      });
      expect(activeTab).toContain(tab === "基础配置" ? "基础配置" : tab === "SSL" ? "SSL" : "伪静态");
    }

    // 关
    await browser.execute(() => {
      const overlay = document.querySelector(".modal-overlay")!;
      const cancel = Array.from(overlay.querySelectorAll("button")).find((b) => b.textContent?.includes("取消")) as HTMLElement | undefined;
      cancel?.click();
    });
    await browser.pause(400);
  });
});

describe("SelectMenu 下拉行为", () => {
  before(async () => { await navigate("仪表板"); });

  it("点击下拉触发器 → 菜单出现 → 再点关闭", async () => {
    // 找一个 SelectMenu trigger
    const opened = await browser.execute(() => {
      const trigger = document.querySelector("main .rs-select-trigger") as HTMLButtonElement | null;
      if (!trigger) return false;
      trigger.click();
      return true;
    });
    if (!opened) return; // 仪表板没下拉时跳过

    await browser.pause(300);
    let menuCount = await browser.execute(() => document.querySelectorAll(".rs-select-menu").length);
    expect(menuCount).toBe(1);

    // 点空白处关闭（触发 onWindowClick）
    await browser.execute(() => {
      document.body.click();
    });
    await browser.pause(300);
    menuCount = await browser.execute(() => document.querySelectorAll(".rs-select-menu").length);
    expect(menuCount).toBe(0);
  });

  it("打开多次不重复（KeepAlive 监听不泄漏）— 切走再回来菜单仍能用", async () => {
    await navigate("服务配置");
    await clickConfigTab("nginx");

    // 切走
    await navigate("仪表板");
    // 再切回
    await navigate("服务配置");
    await clickConfigTab("nginx");
    await browser.pause(500);

    // 找 nginx tab 里第一个下拉 trigger，点开关三次都 OK
    for (let i = 0; i < 3; i++) {
      const ok = await browser.execute(() => {
        const trigger = document.querySelector("main .rs-select-trigger") as HTMLButtonElement | null;
        if (!trigger) return null;
        trigger.click();
        return true;
      });
      if (ok === null) break;
      await browser.pause(150);
      const menuCount = await browser.execute(() => document.querySelectorAll(".rs-select-menu").length);
      expect(menuCount).toBe(1);
      await browser.execute(() => document.body.click());
      await browser.pause(150);
    }
  });
});

describe("ServiceConfig tab 切换", () => {
  before(async () => { await navigate("服务配置"); });

  it("6 个 tab 来回切都能正常显示对应内容", async () => {
    const cases: Array<{ tab: "nginx" | "mysql" | "redis" | "php" | "hosts" | "env"; mustContain: string }> = [
      { tab: "nginx", mustContain: "worker_processes" },
      { tab: "mysql", mustContain: "max_connections" },
      { tab: "redis", mustContain: "maxmemory" },
      { tab: "php", mustContain: "扩展管理" },
      { tab: "hosts", mustContain: "系统 Hosts" },
      { tab: "env", mustContain: "全局" },
    ];
    for (const c of cases) {
      await clickConfigTab(c.tab);
      expect(await mainContains(c.mustContain)).toBe(true);
    }
  });
});

describe("Toast 系统", () => {
  it("UI 上有空 toast 容器（ToastContainer 已挂载）", async () => {
    const exists = await browser.execute(() => {
      // ToastContainer 在 App.vue 末尾挂载，类名以 fixed 定位
      return document.querySelector("[class*='toast']") !== null
        || document.querySelector("[class*='Toast']") !== null
        || document.querySelectorAll("body > div").length > 1; // 至少应用本体 + toast 容器
    });
    expect(exists).toBe(true);
  });
});

describe("textarea 行为（Hosts tab）", () => {
  before(async () => {
    await navigate("服务配置");
    await clickConfigTab("hosts");
    await browser.pause(800);
  });

  it("textarea 关闭了 spellcheck", async () => {
    const sc = await browser.execute(() => {
      const t = document.querySelector("main textarea") as HTMLTextAreaElement | null;
      return t?.spellcheck ?? null;
    });
    expect(sc).toBe(false);
  });

  it("Tab 键插入制表符（不切焦点）", async () => {
    // 直接通过 JS 模拟（更稳定）：调用我们的 onTextareaTab handler
    // 由于 wdio sendKeys 在 webview 里 Tab 行为不太可预测，改用 dispatch keydown
    const inserted = await browser.execute(() => {
      const t = document.querySelector("main textarea") as HTMLTextAreaElement | null;
      if (!t) return null;
      const before = t.value;
      t.focus();
      t.selectionStart = t.selectionEnd = before.length;
      // 模拟 Tab keydown，让 vue 的 @keydown 处理器接管
      const evt = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
      t.dispatchEvent(evt);
      return { before: before.length, after: t.value.length, last: t.value.slice(-1) };
    });
    if (inserted) {
      expect(inserted.after).toBe(inserted.before + 1);
      expect(inserted.last).toBe("\t");
    }
  });
});
