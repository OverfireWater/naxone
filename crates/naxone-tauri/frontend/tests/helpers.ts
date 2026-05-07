/**
 * E2E 测试共享 helpers：导航、等待渲染、批量查询 DOM。
 */
import { browser, $ } from "@wdio/globals";

export async function waitForApp() {
  await $("#app").waitForExist({ timeout: 30_000 });
  await browser.pause(500); // vue 首次渲染缓冲
}

/** 点击侧栏菜单项跳转 */
export async function navigate(label: "仪表板" | "网站" | "软件商店" | "服务配置" | "设置") {
  await browser.execute((target: string) => {
    const items = Array.from(document.querySelectorAll("aside nav > div")) as HTMLElement[];
    const found = items.find((el) => el.textContent?.includes(target));
    found?.click();
  }, label);
  await browser.pause(800); // KeepAlive 切换 + 数据加载
}

/** 切 ServiceConfig 子 tab */
export async function clickConfigTab(tabKey: "nginx" | "mysql" | "redis" | "php" | "hosts" | "env") {
  await browser.execute((k: string) => {
    const tabs = Array.from(document.querySelectorAll(".tab")) as HTMLElement[];
    const labels: Record<string, string> = {
      nginx: "Nginx",
      mysql: "MySQL",
      redis: "Redis",
      php: "PHP",
      hosts: "Hosts",
      env: "全局环境",
    };
    const t = tabs.find((el) => el.textContent?.trim() === labels[k]);
    t?.click();
  }, tabKey);
  await browser.pause(500);
}

/** 当前主区域 main 内可见的所有 <button> 元素的属性快照 */
export async function buttonsSnapshot(): Promise<Array<{ text: string; disabled: boolean; cursor: string; ariaLabel: string | null; visible: boolean; }>> {
  return browser.execute(() => {
    const main = document.querySelector("main");
    if (!main) return [];
    const buttons = Array.from(main.querySelectorAll("button"));
    return buttons.map((b) => {
      const cs = window.getComputedStyle(b);
      return {
        text: (b.textContent || "").trim().replace(/\s+/g, " ").slice(0, 80),
        disabled: b.disabled || b.hasAttribute("disabled"),
        cursor: cs.cursor,
        ariaLabel: b.getAttribute("aria-label"),
        // visible：实际渲染（offsetParent != null 或 fixed/absolute 元素）
        visible: b.offsetParent !== null || cs.position === "fixed" || cs.position === "absolute",
      };
    });
  });
}

/** 当前主区域 main 内可见的所有 <input> / <select> / <textarea> 元素 */
export async function inputsSnapshot(): Promise<Array<{ type: string; placeholder: string; value: string; disabled: boolean; readOnly: boolean; }>> {
  return browser.execute(() => {
    const main = document.querySelector("main");
    if (!main) return [];
    const els = Array.from(main.querySelectorAll("input, textarea, select")) as Array<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>;
    return els.map((el) => ({
      type: el.tagName === "INPUT" ? (el as HTMLInputElement).type : el.tagName.toLowerCase(),
      placeholder: (el as HTMLInputElement).placeholder || "",
      value: (el as HTMLInputElement).value || "",
      disabled: (el as HTMLInputElement).disabled || false,
      readOnly: (el as HTMLInputElement).readOnly || false,
    }));
  });
}

/** main 区域内整段可见文本（去除空白） */
export async function mainText(): Promise<string> {
  return browser.execute(() => {
    const main = document.querySelector("main");
    return (main?.textContent || "").replace(/\s+/g, " ").trim();
  });
}

/** main 区域内是否包含某段文本 */
export async function mainContains(text: string): Promise<boolean> {
  const t = await mainText();
  return t.includes(text);
}

/** 查询某个 class 在 main 内的元素数量 */
export async function countSelector(selector: string): Promise<number> {
  return browser.execute((sel: string) => {
    const main = document.querySelector("main");
    return main ? main.querySelectorAll(sel).length : 0;
  }, selector);
}

/** 关闭可能打开的 modal（按 Escape） */
export async function closeAllModals() {
  await browser.execute(() => {
    const overlay = document.querySelector(".modal-overlay") as HTMLElement | null;
    if (overlay) {
      const cancelBtn = overlay.querySelector(".btn-secondary") as HTMLElement | null;
      cancelBtn?.click();
    }
  });
  await browser.pause(300);
}
