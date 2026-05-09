/**
 * v0.5.11 模板失败 UI 行为测试（GUI 流程）
 *
 * 锁定 bug：模板初始化失败（如目录非空）时，UI 不应再显示成功后才该出现的内容
 * （比如 webman cli 启动提示、入口子目录调整提示）。
 */
import { browser, expect, $ } from "@wdio/globals";
import { waitForApp, navigate } from "../helpers.js";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";

declare global {
  interface Window {
    __TAURI__?: {
      core: { invoke: <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T> };
    };
  }
}

const TMP_ROOT = path.join(os.tmpdir(), "naxone-template-fail-ui");
const TEST_DOMAIN = `e2e-fail-${Date.now().toString(36)}.local`;
const TEST_PORT = 80;
const TEST_VHOST_ID = `${TEST_DOMAIN}_${TEST_PORT}`;

describe("v0.5.11 模板失败 UI 行为", () => {
  before(async () => {
    await waitForApp();
    if (fs.existsSync(TMP_ROOT)) fs.rmSync(TMP_ROOT, { recursive: true, force: true });
    fs.mkdirSync(TMP_ROOT, { recursive: true });
    fs.writeFileSync(path.join(TMP_ROOT, "preexisting.txt"), "占位让目录非空");
    await browser.setTimeout({ script: 60_000 });
  });

  after(async () => {
    try {
      await browser.execute(async (id: string) => {
        await window.__TAURI__!.core.invoke("delete_vhost", { id });
      }, TEST_VHOST_ID);
    } catch {}
    if (fs.existsSync(TMP_ROOT)) {
      try { fs.rmSync(TMP_ROOT, { recursive: true, force: true }); } catch {}
    }
  });

  it("Webman 模板装非空目录 → modal 含'目录非空'但不含 cli 启动提示", async () => {
    await navigate("网站");

    // 点 + 新建站点
    await browser.execute(() => {
      const btns = Array.from(document.querySelectorAll("button")) as HTMLButtonElement[];
      btns.find((b) => b.textContent?.includes("新建站点"))?.click();
    });
    await browser.pause(400);

    // 填域名 + 文档目录（指向非空目录）
    await browser.execute((domain: string, root: string) => {
      const inputs = Array.from(document.querySelectorAll(".modal-content input.input")) as HTMLInputElement[];
      const setVal = (el: HTMLInputElement, val: string) => {
        el.value = val;
        el.dispatchEvent(new Event("input", { bubbles: true }));
      };
      // 表单顺序：[0] 域名 [1] 端口 [2] 别名 [3] 文档目录
      setVal(inputs[0], domain);
      setVal(inputs[3], root);
    }, TEST_DOMAIN, TMP_ROOT);
    await browser.pause(200);

    // 选模板下拉 = Webman（找包含"初始化模板"label 的 .fg → 兄弟 trigger）
    await browser.execute(() => {
      const labels = Array.from(document.querySelectorAll(".modal-content .fg label"));
      const label = labels.find((l) => l.textContent?.includes("初始化模板"));
      const trigger = label?.parentElement?.querySelector(".rs-select-trigger") as HTMLButtonElement | null;
      trigger?.click();
    });
    await browser.pause(300);

    // 点菜单中的 Webman 选项
    await browser.execute(() => {
      const items = Array.from(document.querySelectorAll(".rs-select-menu .rs-select-option")) as HTMLButtonElement[];
      items.find((i) => i.textContent?.includes("Webman"))?.click();
    });
    await browser.pause(300);

    // 点保存
    await browser.execute(() => {
      const btns = Array.from(document.querySelectorAll(".modal-content button")) as HTMLButtonElement[];
      btns.find((b) => b.textContent?.trim() === "保存")?.click();
    });

    // 等模板 modal 出现（标题"正在初始化站点…"或"初始化完成"）
    await $("//*[contains(text(), '正在初始化站点') or contains(text(), '初始化完成')]").waitForExist({ timeout: 30_000 });

    // 等模板进程结束（"关闭"按钮变可点）
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const btns = Array.from(document.querySelectorAll("button")) as HTMLButtonElement[];
          const closeBtn = btns.find((b) => b.textContent?.trim() === "关闭");
          return !!closeBtn && !closeBtn.disabled;
        }),
      { timeout: 30_000, timeoutMsg: "模板初始化未结束" },
    );

    // 读 modal pre 全部文本
    const logs = await browser.execute(() => {
      const pre = document.querySelector(".modal-content pre") as HTMLElement | null;
      return pre?.textContent || "";
    });

    // 关键断言：失败信号有，成功后才该有的提示无
    expect(logs).toContain("目录非空");
    expect(logs).not.toContain("Webman 是常驻 cli 进程");
    expect(logs).not.toContain("启动后浏览器访问本站点即可");
  });
});
