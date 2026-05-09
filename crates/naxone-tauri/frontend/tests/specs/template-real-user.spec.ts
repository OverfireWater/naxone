/**
 * v0.5.11 真实用户流程测试 — Webman 模板
 *
 * Webman 是用户实测时报"目录非空"的原始 bug 场景，专门用它做端到端真实路径锁住 fix。
 * 完整覆盖 NaxOne 自家流程：create_vhost 会先在 document_root 写 nginx.htaccess + .htaccess，
 * 再 init_site_template。修复后 init 必须忽略这两个自家文件。
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

const TS = Date.now().toString(36);
const WM_DIR = path.join(os.tmpdir(), `naxone-real-webman-${TS}`);
const WM_DOMAIN = `e2e-webman-${TS}.local`;
const WM_PORT = 80;
const WM_VHOST_ID = `${WM_DOMAIN}_${WM_PORT}`;

/// 清理上次 wdio 测试残留的 vhost conf 文件（after hook 失败时会留下来 → 下次 nginx reload 报 emerg）
function cleanupOrphanVhostConfs() {
  const candidates = [
    "D:/phpstudy_pro/Extensions/Nginx1.15.11/conf/vhosts",
    "D:/phpstudy_pro/Extensions/Apache2.4.39/conf/vhosts",
  ];
  for (const dir of candidates) {
    if (!fs.existsSync(dir)) continue;
    for (const f of fs.readdirSync(dir)) {
      if (/^e2e-/.test(f)) {
        try { fs.unlinkSync(path.join(dir, f)); } catch {}
      }
    }
  }
}

describe("v0.5.11 真实用户流程：Webman 模板（bug 原始场景）", () => {
  before(async () => {
    cleanupOrphanVhostConfs();
    await waitForApp();
    if (fs.existsSync(WM_DIR)) fs.rmSync(WM_DIR, { recursive: true, force: true });
    await browser.setTimeout({ script: 360_000 });
  });

  after(async () => {
    try {
      await browser.execute(async (id: string) => {
        await window.__TAURI__!.core.invoke("delete_vhost", { id });
      }, WM_VHOST_ID);
    } catch {}
    if (fs.existsSync(WM_DIR)) {
      try { fs.rmSync(WM_DIR, { recursive: true, force: true }); } catch {}
    }
  });

  it("点 + 新建站点 → 选 Webman → composer 装 → start.php 真实落盘", async function () {
    this.timeout(360_000);
    await navigate("网站");

    // 点 + 新建站点
    await browser.execute(() => {
      const btns = Array.from(document.querySelectorAll("button")) as HTMLButtonElement[];
      btns.find((b) => b.textContent?.includes("新建站点"))?.click();
    });
    await browser.pause(500);

    // 填表单：域名 + 端口 + 文档目录 + 关掉同步 Hosts（避免 UAC 弹窗卡住测试）
    await browser.execute((domain: string, port: number, root: string) => {
      const inputs = Array.from(document.querySelectorAll(".modal-content input.input")) as HTMLInputElement[];
      const setVal = (el: HTMLInputElement, val: string) => {
        el.value = val;
        el.dispatchEvent(new Event("input", { bubbles: true }));
      };
      setVal(inputs[0], domain);
      setVal(inputs[1], String(port));
      setVal(inputs[3], root);
      // 关闭 sync_hosts toggle —— 自动化跑不了 UAC 弹窗
      const syncToggle = document.querySelector(".modal-content input[type=checkbox]") as HTMLInputElement | null;
      if (syncToggle && syncToggle.checked) syncToggle.click();
    }, WM_DOMAIN, WM_PORT, WM_DIR);
    await browser.pause(300);

    // 选模板下拉 → Webman
    await browser.execute(() => {
      const labels = Array.from(document.querySelectorAll(".modal-content .fg label"));
      const label = labels.find((l) => l.textContent?.includes("初始化模板"));
      const trigger = label?.parentElement?.querySelector(".rs-select-trigger") as HTMLButtonElement | null;
      trigger?.click();
    });
    await browser.pause(400);

    await browser.execute(() => {
      const items = Array.from(document.querySelectorAll(".rs-select-menu .rs-select-option")) as HTMLButtonElement[];
      items.find((i) => i.textContent?.includes("Webman"))?.click();
    });
    await browser.pause(400);

    // DEBUG: 点保存前快照表单状态
    const formSnap = await browser.execute(() => {
      const inputs = Array.from(document.querySelectorAll(".modal-content input.input")) as HTMLInputElement[];
      const triggers = Array.from(document.querySelectorAll(".modal-content .rs-select-trigger")) as HTMLElement[];
      return {
        domain: inputs[0]?.value,
        port: inputs[1]?.value,
        docRoot: inputs[3]?.value,
        // SelectMenu 显示标签
        triggerLabels: triggers.map((t) => t.textContent?.trim()),
      };
    });
    console.log("[DEBUG] form snapshot:", JSON.stringify(formSnap));

    // 点保存
    await browser.execute(() => {
      const btns = Array.from(document.querySelectorAll(".modal-content button")) as HTMLButtonElement[];
      btns.find((b) => b.textContent?.trim() === "保存")?.click();
    });
    await browser.pause(1000);

    // 等模板 modal 出现
    await $("//*[contains(text(), '正在初始化站点') or contains(text(), '初始化完成')]").waitForExist({ timeout: 30_000 });

    // composer create-project workerman/webman 实测 30-90 秒，给 5 分钟兜底
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const btns = Array.from(document.querySelectorAll("button")) as HTMLButtonElement[];
          const closeBtn = btns.find((b) => b.textContent?.trim() === "关闭");
          return !!closeBtn && !closeBtn.disabled;
        }),
      { timeout: 300_000, timeoutMsg: "Webman 模板初始化未在 5 分钟内完成" },
    );

    const logs = await browser.execute(() => {
      const pre = document.querySelector(".modal-content pre") as HTMLElement | null;
      return pre?.textContent || "";
    });

    // 核心 bug fix 断言（不依赖 composer 网络）：
    // 1) "目录非空" 不出现 —— NaxOne 写的 htaccess 没让 init 拒绝
    // 2) "开始初始化模板" 出现 —— 流程顺利推进到 composer 阶段
    // 3) nginx.htaccess 文件存在 —— 证明 create_vhost 真的写过 htaccess
    expect(logs).not.toContain("目录非空");
    expect(logs).toContain("开始初始化模板");
    expect(fs.existsSync(path.join(WM_DIR, "nginx.htaccess"))).toBe(true);

    // 软断言：composer 网络 OK 时验证 webman 关键文件
    if (logs.includes("✔ 初始化完成") || (!logs.includes("初始化失败") && !logs.includes("composer create-project 失败"))) {
      expect(fs.existsSync(path.join(WM_DIR, "start.php"))).toBe(true);
      expect(fs.existsSync(path.join(WM_DIR, "windows.php"))).toBe(true);
      expect(fs.existsSync(path.join(WM_DIR, "vendor"))).toBe(true);
      expect(fs.existsSync(path.join(WM_DIR, "app"))).toBe(true);
      expect(fs.existsSync(path.join(WM_DIR, "config"))).toBe(true);
      expect(fs.existsSync(path.join(WM_DIR, "public"))).toBe(true);
    } else {
      console.log("⚠ Webman composer 装包失败（可能是网络/composer 环境），仅验证 bug fix 路径");
    }
  });
});
