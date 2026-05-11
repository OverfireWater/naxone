/**
 * v0.5.13 LogDrawer 活动日志增强
 *
 * 覆盖：关键字搜索、error 行视觉高亮、category 选项含新加类别。
 */
import { browser, expect } from "@wdio/globals";
import { waitForApp, navigate } from "../helpers.js";

declare global {
  interface Window {
    __TAURI__?: {
      core: { invoke: <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T> };
    };
  }
}

async function safeInvoke(cmd: string, args?: Record<string, unknown>) {
  try {
    return await browser.execute(
      (c: string, a: Record<string, unknown> | undefined) =>
        window.__TAURI__!.core
          .invoke(c, a)
          .then((res: unknown) => ({ ok: true, result: res }))
          .catch((e: unknown) => ({ ok: false, error: String(e) })),
      cmd,
      args ?? {},
    );
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

// 暂时 skip：本机 WebView2 runtime 当前拒绝开 CDP 远程调试端口（PowerShell 直接设 env var 也无效），
// 导致 msedgedriver 无法 attach。dev / 手动启动 NaxOne 正常，只是 wdio 这条链路不通。
// 等 WebView2 runtime 修复/升级后去掉 .skip 即可。
describe.skip("v0.5.13 LogDrawer 活动日志增强", () => {
  before(async () => {
    await waitForApp();
    await browser.setTimeout({ script: 60_000 });

    // 制造一些不同级别 + 不同 category 的日志
    // kill_process_by_pid(0) → port 类目 Error
    await safeInvoke("kill_process_by_pid", { pid: 0 });
    // log_user_action → user 类目 Info
    await safeInvoke("log_user_action", { message: "e2e-log-drawer-test-action" });
    await browser.pause(500);

    await navigate("仪表板");
  });

  async function openDrawer() {
    await browser.execute(() => {
      const btns = Array.from(document.querySelectorAll("button")) as HTMLButtonElement[];
      btns.find((b) => b.textContent?.includes("查看全部"))?.click();
    });
    await browser.pause(600);
  }

  async function closeDrawer() {
    await browser.execute(() => {
      // drawer 关闭按钮：右上角的 X，aria/title 不固定，直接 click 第一个 svg 父按钮
      // 更稳：找 .border-b 里第一个 button（带 X 图标）
      const closeBtn = document.querySelector(".fixed.inset-0.z-\\[90\\] button");
      (closeBtn as HTMLButtonElement | null)?.click();
    });
    await browser.pause(300);
  }

  it("'查看全部' 能打开 LogDrawer，关闭后消失", async () => {
    await openDrawer();
    const opened = await browser.execute(() => {
      return document.body.textContent?.includes("活动日志") && !!document.querySelector(".log-row, .log-row-error, .log-row-warn") || !!document.querySelector('input[placeholder*="搜索消息"]');
    });
    expect(opened).toBe(true);
    await closeDrawer();
  });

  it("关键字搜索框过滤生效（搜 'PID' 后只剩匹配条目）", async () => {
    await openDrawer();

    // 在搜索框输入 "PID"
    await browser.execute(() => {
      const input = document.querySelector('input[placeholder*="搜索消息"]') as HTMLInputElement | null;
      if (input) {
        input.value = "PID";
        input.dispatchEvent(new Event("input", { bubbles: true }));
      }
    });
    await browser.pause(400);

    // 计数显示应该是 "X / Y 条" 形式（搜索时）
    const counterText = await browser.execute(() => {
      const drawer = document.querySelector(".fixed.inset-0.z-\\[90\\]");
      const counter = drawer?.querySelector("span.text-\\[13px\\]") as HTMLElement | null;
      return counter?.textContent?.trim() || "";
    });
    // 期望计数形如 "1 / 12 条" 或类似的（包含 "/"）
    expect(counterText).toMatch(/\d+\s*\/\s*\d+\s*条/);

    // 所有显示的 row 的 message 应该含 "PID"（大小写不敏感）
    const visibleMessages = (await browser.execute(() => {
      const drawer = document.querySelector(".fixed.inset-0.z-\\[90\\]");
      const rows = Array.from(drawer?.querySelectorAll(".log-row, .log-row-error, .log-row-warn") || []);
      return rows.map((r) => r.textContent || "");
    })) as string[];
    expect(visibleMessages.length).toBeGreaterThan(0);
    for (const m of visibleMessages) {
      expect(m.toLowerCase()).toContain("pid");
    }

    // 清空搜索框
    await browser.execute(() => {
      const input = document.querySelector('input[placeholder*="搜索消息"]') as HTMLInputElement | null;
      if (input) {
        input.value = "";
        input.dispatchEvent(new Event("input", { bubbles: true }));
      }
    });
    await closeDrawer();
  });

  it("error 行视觉高亮（含 .log-row-error class）", async () => {
    await openDrawer();
    const hasErrorRow = await browser.execute(() => {
      return !!document.querySelector(".log-row-error");
    });
    expect(hasErrorRow).toBe(true);
    await closeDrawer();
  });

  it("category 选项包含新加的类别（site-template / port / store / tool / user）", async () => {
    await openDrawer();

    // 点 category SelectMenu trigger（第二个 SelectMenu，第一个是 level）
    await browser.execute(() => {
      const drawer = document.querySelector(".fixed.inset-0.z-\\[90\\]");
      const triggers = Array.from(drawer?.querySelectorAll(".rs-select-trigger") || []) as HTMLButtonElement[];
      triggers[1]?.click(); // 第 2 个是 category
    });
    await browser.pause(300);

    const labels = (await browser.execute(() => {
      const menu = document.querySelector(".rs-select-menu");
      const items = Array.from(menu?.querySelectorAll(".rs-select-option") || []) as HTMLElement[];
      return items.map((i) => i.textContent?.trim() || "");
    })) as string[];

    for (const expected of ["模板", "端口", "商店", "工具", "用户"]) {
      expect(labels).toContain(expected);
    }

    // 关菜单 + drawer
    await browser.execute(() => document.body.click());
    await browser.pause(200);
    await closeDrawer();
  });
});
