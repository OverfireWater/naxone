/**
 * 截图脚本：跑过整个应用，把每个页面/状态截屏保存到 docs/screenshots/。
 * 不是断言测试，是文档生成器。最后一个 it() 总是通过。
 *
 * 跑：npm run test:e2e -- --spec tests/specs/screenshots.spec.ts
 */
import { browser } from "@wdio/globals";
import { expect } from "@wdio/globals";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { waitForApp, navigate, clickConfigTab } from "../helpers.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
// __dirname = frontend/tests/specs → ruststudy 要 5 层 ../
const OUT = path.resolve(__dirname, "../../../../../docs/screenshots");
fs.mkdirSync(OUT, { recursive: true });

async function shot(name: string) {
  await browser.pause(700); // 给 vue + 动画稳定时间
  const file = path.join(OUT, `${name}.png`);
  await browser.saveScreenshot(file);
  console.log("saved:", file);
}

describe("📸 截图生成（docs/screenshots/）", () => {
  before(async () => {
    await waitForApp();
    // 设窗口尺寸固定，截图一致
    await browser.setWindowSize(1280, 820);
    await browser.pause(500);
  });

  it("01-dashboard 仪表板", async () => {
    await navigate("仪表板");
    await shot("01-dashboard");
  });

  it("02-vhosts-list 网站列表", async () => {
    await navigate("网站");
    await shot("02-vhosts-list");
  });

  it("03-vhosts-modal-basic 新建站点-基础配置", async () => {
    await browser.execute(() => {
      const buttons = Array.from(document.querySelectorAll("main button")) as HTMLElement[];
      buttons.find((b) => b.textContent?.includes("新建站点"))?.click();
    });
    await shot("03-vhosts-modal-basic");
  });

  it("04-vhosts-modal-rewrite 新建站点-伪静态", async () => {
    await browser.execute(() => {
      const tabs = Array.from(document.querySelectorAll(".modal-content .modal-tab")) as HTMLElement[];
      tabs.find((t) => t.textContent?.includes("伪静态"))?.click();
    });
    await shot("04-vhosts-modal-rewrite");
  });

  it("05-vhosts-modal-ssl 新建站点-SSL 与高级", async () => {
    await browser.execute(() => {
      const tabs = Array.from(document.querySelectorAll(".modal-content .modal-tab")) as HTMLElement[];
      tabs.find((t) => t.textContent?.includes("SSL"))?.click();
    });
    await shot("05-vhosts-modal-ssl");
    // 关 modal
    await browser.execute(() => {
      const overlay = document.querySelector(".modal-overlay")!;
      const cancel = Array.from(overlay.querySelectorAll("button")).find((b) => b.textContent?.includes("取消")) as HTMLElement | undefined;
      cancel?.click();
    });
    await browser.pause(400);
  });

  it("06-store 软件商店", async () => {
    await navigate("软件商店");
    await shot("06-store");
  });

  it("07-config-env 服务配置·全局环境", async () => {
    await navigate("服务配置");
    await clickConfigTab("env");
    await shot("07-config-env");
  });

  it("08-config-nginx 服务配置·Nginx", async () => {
    await clickConfigTab("nginx");
    await shot("08-config-nginx");
  });

  it("09-config-mysql 服务配置·MySQL", async () => {
    await clickConfigTab("mysql");
    await shot("09-config-mysql");
  });

  it("10-config-redis 服务配置·Redis", async () => {
    await clickConfigTab("redis");
    await shot("10-config-redis");
  });

  it("11-config-php 服务配置·PHP", async () => {
    await clickConfigTab("php");
    await shot("11-config-php");
  });

  it("12-config-hosts 服务配置·Hosts", async () => {
    await clickConfigTab("hosts");
    await shot("12-config-hosts");
  });

  it("13-settings 设置", async () => {
    await navigate("设置");
    await shot("13-settings");
  });

  it("最终：列出已生成的截图", async () => {
    const files = fs.readdirSync(OUT).filter((f) => f.endsWith(".png")).sort();
    console.log(`\n生成 ${files.length} 张截图于 ${OUT}：`);
    files.forEach((f) => console.log("  -", f));
    expect(files.length).toBeGreaterThan(0);
  });
});
