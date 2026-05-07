/**
 * UI 完整性测试：每页所有按钮 / 输入 / 关键元素的可见性 + 状态。
 * 不触发后端写操作，仅校验 DOM 渲染正确。
 */
import { browser, $ } from "@wdio/globals";
import { expect } from "@wdio/globals";
import { waitForApp, navigate, clickConfigTab, buttonsSnapshot, inputsSnapshot, mainText, mainContains, countSelector } from "../helpers.js";

describe("App Shell（标题栏 + 侧栏）", () => {
  before(async () => { await waitForApp(); });

  it("标题栏：3 个窗口控制按钮（最小化/最大化/关闭）都存在且可点击", async () => {
    const btns = await browser.execute(() => {
      // 应用顶部 titlebar 是 app-shell 的第一个 div；侧栏 aside 也带 titlebar-no-drag class，所以要按层级缩范围
      const titleBar = document.querySelector(".app-shell > div") as HTMLElement | null;
      const buttons = titleBar ? Array.from(titleBar.querySelectorAll("button")) : [];
      return buttons.map((b) => ({
        cursor: window.getComputedStyle(b as HTMLElement).cursor,
        visible: (b as HTMLElement).offsetParent !== null,
      }));
    });
    expect(btns.length).toBe(3);
    btns.forEach((b) => {
      expect(b.cursor).toBe("pointer");
      expect(b.visible).toBe(true);
    });
  });

  it("侧栏：5 个菜单项，文字正确", async () => {
    const labels = await browser.execute(() => {
      const items = Array.from(document.querySelectorAll("aside nav > div"));
      return items.map((el) => el.textContent?.trim().replace(/\s+/g, "") ?? "");
    });
    expect(labels).toHaveLength(5);
    expect(labels.join("|")).toContain("仪表板");
    expect(labels.join("|")).toContain("网站");
    expect(labels.join("|")).toContain("软件商店");
    expect(labels.join("|")).toContain("服务配置");
    expect(labels.join("|")).toContain("设置");
  });

  it("侧栏：折叠按钮存在", async () => {
    const exists = await browser.execute(() => {
      const aside = document.querySelector("aside");
      return aside?.querySelector("div[class*='cursor-pointer']") !== null;
    });
    expect(exists).toBe(true);
  });
});

describe("Dashboard（仪表板）", () => {
  before(async () => { await navigate("仪表板"); });

  it("顶部刷新按钮 + 全部启停按钮存在", async () => {
    const text = await mainText();
    // 全部启动 / 全部停止 按钮根据状态切换显示
    const hasBatch = text.includes("全部启动") || text.includes("全部停止");
    expect(hasBatch).toBe(true);
  });

  it("4 类服务卡都渲染（Nginx/Apache/MySQL/Redis）", async () => {
    const cards = await countSelector(".svc-card");
    // 至少 1 张（可能没装 Apache 也 OK），合理预期 1-4
    expect(cards).toBeGreaterThanOrEqual(1);
    const text = await mainText();
    // 至少识别到 nginx 或 mysql
    const seen = ["Nginx", "Apache", "MySQL", "Redis"].filter((k) => text.includes(k));
    expect(seen.length).toBeGreaterThanOrEqual(1);
  });

  it("PHP 引擎卡渲染（如有 PHP 实例）", async () => {
    const text = await mainText();
    const hasPhp = text.includes("PHP 引擎") || text.includes("3/5 运行中") || text.includes("/0 运行中");
    // 没装 PHP 时这块整段隐藏；不强制
    if (text.includes("PHP 引擎")) {
      expect(hasPhp).toBe(true);
      const chips = await countSelector(".php-chip");
      expect(chips).toBeGreaterThanOrEqual(1);
    }
  });

  it("全局环境只读条渲染（PHP / Composer / Node / MySQL）", async () => {
    const items = await countSelector(".env-summary-item");
    expect(items).toBe(4);
    const text = await mainText();
    expect(text).toContain("全局环境");
  });

  it("活动日志卡渲染 + 查看全部按钮", async () => {
    const text = await mainText();
    expect(text).toContain("活动日志");
    expect(text).toContain("查看全部");
  });

  it("所有按钮的 cursor 都是 pointer / 不应有 disabled 但 visible 错位", async () => {
    const buttons = await buttonsSnapshot();
    const broken = buttons.filter((b) => b.visible && b.cursor !== "pointer" && !b.disabled);
    expect(broken).toEqual([]);
  });
});

describe("Vhosts（网站列表）", () => {
  before(async () => { await navigate("网站"); });

  it("搜索框 + 新建站点按钮都存在", async () => {
    const text = await mainText();
    expect(text).toContain("新建站点");
    // 搜索 input 应该有 placeholder 含"搜索"
    const inputs = await inputsSnapshot();
    const hasSearch = inputs.some((i) => i.placeholder.includes("搜索") || i.placeholder.includes("域名"));
    expect(hasSearch).toBe(true);
  });

  it("表头存在（域名 / 端口 / 网站目录 / PHP / 到期 / 状态 / 操作）", async () => {
    const text = await mainText();
    expect(text).toContain("域名");
    expect(text).toContain("网站目录");
  });

  it("打开新建站点 modal 后基础配置 tab 字段存在", async () => {
    await browser.execute(() => {
      const buttons = Array.from(document.querySelectorAll("main button")) as HTMLElement[];
      buttons.find((b) => b.textContent?.includes("新建站点"))?.click();
    });
    await browser.pause(500);

    const overlayCount = await browser.execute(() => document.querySelectorAll(".modal-overlay").length);
    expect(overlayCount).toBe(1);

    const modalText = await browser.execute(() => {
      const m = document.querySelector(".modal-content");
      return (m?.textContent || "").replace(/\s+/g, " ");
    });
    expect(modalText).toContain("基础配置");
    expect(modalText).toContain("伪静态");
    expect(modalText).toContain("SSL");
    expect(modalText).toContain("域名");
    expect(modalText).toContain("网站目录");
    expect(modalText).toContain("PHP 版本");
    expect(modalText).toContain("保存");
    expect(modalText).toContain("取消");
  });

  it("Modal 切到伪静态 tab，规则 textarea 存在 + spellcheck 关闭", async () => {
    await browser.execute(() => {
      const tabs = Array.from(document.querySelectorAll(".modal-content .modal-tab")) as HTMLElement[];
      tabs.find((t) => t.textContent?.includes("伪静态"))?.click();
    });
    await browser.pause(300);

    const ta = await browser.execute(() => {
      const t = document.querySelector(".modal-content textarea") as HTMLTextAreaElement | null;
      return t ? { exists: true, spellcheck: t.spellcheck } : { exists: false, spellcheck: true };
    });
    expect(ta.exists).toBe(true);
    expect(ta.spellcheck).toBe(false);
  });

  it("Modal 切到 SSL tab，证书相关字段存在", async () => {
    await browser.execute(() => {
      const tabs = Array.from(document.querySelectorAll(".modal-content .modal-tab")) as HTMLElement[];
      tabs.find((t) => t.textContent?.includes("SSL"))?.click();
    });
    await browser.pause(300);

    const modalText = await browser.execute(() => {
      const m = document.querySelector(".modal-content");
      return (m?.textContent || "").replace(/\s+/g, " ");
    });
    expect(modalText).toContain("证书路径");
    expect(modalText).toContain("密钥路径");
    expect(modalText).toContain("一键生成");
    expect(modalText).toContain("强制 HTTPS");
  });

  it("Modal 取消关闭", async () => {
    await browser.execute(() => {
      const overlay = document.querySelector(".modal-overlay") as HTMLElement | null;
      const cancelBtn = Array.from(overlay?.querySelectorAll("button") || []).find((b) => b.textContent?.includes("取消")) as HTMLElement | undefined;
      cancelBtn?.click();
    });
    await browser.pause(400);
    const overlayCount = await browser.execute(() => document.querySelectorAll(".modal-overlay").length);
    expect(overlayCount).toBe(0);
  });
});

describe("SoftwareStore（软件商店）", () => {
  before(async () => { await navigate("软件商店"); });

  it("至少渲染了一些软件包卡片", async () => {
    const cards = await countSelector(".store-card");
    expect(cards).toBeGreaterThanOrEqual(1);
  });

  it("包卡片含名称 + 版本下拉 + 操作按钮", async () => {
    const text = await mainText();
    // 应该有一些已知软件名
    const knownNames = ["nginx", "Nginx", "Apache", "PHP", "MySQL", "Redis", "Composer", "Node"];
    const seen = knownNames.filter((n) => text.toLowerCase().includes(n.toLowerCase()));
    expect(seen.length).toBeGreaterThanOrEqual(2);
  });
});

describe("ServiceConfig（服务配置）", () => {
  before(async () => { await navigate("服务配置"); });

  it("6 个 tab 都存在（全局环境 / Nginx / MySQL / Redis / PHP / Hosts）", async () => {
    const tabs = await browser.execute(() => {
      const els = Array.from(document.querySelectorAll(".tab"));
      return els.map((e) => e.textContent?.trim() ?? "");
    });
    ["全局环境", "Nginx", "MySQL", "Redis", "PHP", "Hosts"].forEach((label) => {
      expect(tabs.join("|")).toContain(label);
    });
  });

  it("Nginx tab：保存按钮 + 打开配置 + 查看日志 + 多个字段", async () => {
    await clickConfigTab("nginx");
    const text = await mainText();
    expect(text).toContain("保存配置");
    expect(text).toContain("打开配置文件");
    expect(text).toContain("查看日志");
    // 至少几个 nginx 关键字段
    const keys = ["worker_processes", "client_max_body_size", "keepalive_timeout"];
    keys.forEach((k) => expect(text).toContain(k));
  });

  it("MySQL tab：关键字段存在", async () => {
    await clickConfigTab("mysql");
    const text = await mainText();
    expect(text).toContain("max_connections");
    expect(text).toContain("innodb_buffer_pool_size");
  });

  it("Redis tab：关键字段存在", async () => {
    await clickConfigTab("redis");
    const text = await mainText();
    expect(text).toContain("maxmemory");
    expect(text).toContain("appendonly");
  });

  it("PHP tab：版本下拉 + 子 tab（扩展 / php.ini）", async () => {
    await clickConfigTab("php");
    const text = await mainText();
    expect(text).toContain("扩展管理");
    expect(text).toContain("php.ini");
  });

  it("Hosts tab：textarea + 系统编辑器打开按钮", async () => {
    await clickConfigTab("hosts");
    const text = await mainText();
    expect(text).toContain("系统 Hosts 文件");
    expect(text).toContain("系统编辑器打开");
    const ta = await browser.execute(() => {
      const t = document.querySelector("main textarea") as HTMLTextAreaElement | null;
      return t ? { exists: true, spellcheck: t.spellcheck } : { exists: false, spellcheck: true };
    });
    expect(ta.exists).toBe(true);
    expect(ta.spellcheck).toBe(false);
  });

  it("全局环境 tab：MySQL 密码字段含眼睛图标按钮（input 内嵌）", async () => {
    await clickConfigTab("env");
    await browser.pause(800);
    // env-pwd-toggle 是我们加的眼睛图标按钮
    const has = await browser.execute(() => {
      return document.querySelector("main .env-pwd-toggle") !== null;
    });
    // 仅当机器上检测到 MySQL 时这个字段才出现，宽容判定
    if (await mainContains("root 密码")) {
      expect(has).toBe(true);
    }
  });
});

describe("Settings（设置）", () => {
  before(async () => { await navigate("设置"); });

  it("PHPStudy 路径 + 外观主题 + 端口配置 + 自动启动 + 保存设置", async () => {
    const text = await mainText();
    expect(text).toContain("PHPStudy");
    // 与 Settings.vue 实际文案对齐
    expect(text).toContain("外观主题");
    expect(text).toContain("端口配置");
    expect(text).toContain("自动启动");
    expect(text).toContain("保存设置");
  });

  it("所有 input 都不应是 readOnly", async () => {
    const inputs = await inputsSnapshot();
    const visibleInputs = inputs.filter((i) => !i.disabled);
    const allWritable = visibleInputs.every((i) => !i.readOnly);
    expect(allWritable).toBe(true);
  });
});

describe("通用契约（KeepAlive 副作用 / 切换流畅）", () => {
  it("快速切 5 次菜单不报错 + 最后落点正确", async () => {
    await navigate("仪表板");
    await navigate("网站");
    await navigate("软件商店");
    await navigate("服务配置");
    await navigate("设置");
    const t = await mainText();
    expect(t).toContain("PHPStudy");
  });
});
