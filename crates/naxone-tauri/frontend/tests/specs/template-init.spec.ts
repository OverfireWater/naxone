/**
 * v0.5.11 站点模板初始化测试
 *
 * 验证 init_site_template 命令对 4 类模板的真实副作用：
 * - blank → 写入 index.php 含 phpinfo
 * - wordpress → 下载并解压 25MB+ zip，关键文件齐
 * - 目录非空 → 后端拒绝
 *
 * Composer 类（laravel / thinkphp）跳过：依赖 composer + 网络，环境不可控。
 */
import { browser } from "@wdio/globals";
import { expect } from "@wdio/globals";
import { waitForApp } from "../helpers.js";
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

const TMP_ROOT = path.join(os.tmpdir(), "naxone-template-test");

function mkEmptyDir(name: string): string {
  const dir = path.join(TMP_ROOT, name);
  if (fs.existsSync(dir)) fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

describe("v0.5.11 站点模板初始化", () => {
  before(async () => {
    await waitForApp();
    // composer create-project 可能 3-5 分钟，把脚本超时拉到 10 分钟兜底
    await browser.setTimeout({ script: 600_000 });
  });

  after(() => {
    // 清理临时目录
    if (fs.existsSync(TMP_ROOT)) {
      try { fs.rmSync(TMP_ROOT, { recursive: true, force: true }); } catch {}
    }
  });

  // ───────────────────────────────────────
  describe("[1] 空白模板", () => {
    const dir = mkEmptyDir("blank");

    it("init_site_template(blank) 成功", async () => {
      const r = await browser.execute(async (target: string) => {
        try {
          await window.__TAURI__!.core.invoke("init_site_template", {
            targetDir: target,
            template: "blank",
          });
          return { ok: true };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      }, dir);
      expect(r.ok).toBe(true);
    });

    it("index.php 写入磁盘且含 phpinfo()", () => {
      const file = path.join(dir, "index.php");
      expect(fs.existsSync(file)).toBe(true);
      const content = fs.readFileSync(file, "utf8");
      expect(content).toContain("phpinfo()");
    });
  });

  // ───────────────────────────────────────
  describe("[2] WordPress 模板（实测下载+解压）", () => {
    const dir = mkEmptyDir("wordpress");

    it("init_site_template(wordpress) 完整下载并解压", async () => {
      const r = await browser.execute(async (target: string) => {
        try {
          await window.__TAURI__!.core.invoke("init_site_template", {
            targetDir: target,
            template: "wordpress",
          });
          return { ok: true };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      }, dir);
      expect(r.ok).toBe(true);
    });

    it("WordPress 核心文件齐", () => {
      // 顶层 wordpress/ 应被剥掉，文件直接落到 dir 下
      expect(fs.existsSync(path.join(dir, "wp-load.php"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "wp-config-sample.php"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "wp-admin"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "wp-content"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "wp-includes"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "index.php"))).toBe(true);
    });

    it("解压后是中文版（看 wp-config-sample 的 zh_CN 提示）", () => {
      // 中文 zip 在 wp-config-sample.php 里默认 WPLANG 或注释里有中文
      // 简单做法：检查 wp-includes/version.php 中 $wp_local_package 含 zh_CN
      const versionFile = path.join(dir, "wp-includes", "version.php");
      expect(fs.existsSync(versionFile)).toBe(true);
      const content = fs.readFileSync(versionFile, "utf8");
      expect(content).toContain("zh_CN");
    });
  });

  // ───────────────────────────────────────
  // ThinkPHP / Laravel 用例已用直接命令行验证可装（见 commit 历史），
  // 但 wdio 内 composer 实装 4-5 分钟，多个 timeout 难同时调，且会让整套 e2e 不稳。
  // 保留代码作参考，默认 skip。需要单独跑去掉 .skip。
  describe.skip("[5] ThinkPHP 模板（实测 composer create-project）", () => {
    const dir = mkEmptyDir("thinkphp");

    it("init_site_template(thinkphp) 完成", async function () {
      this.timeout(300_000);
      let r: { ok: boolean; error?: string };
      try {
        r = await browser.execute((target: string) =>
          window.__TAURI__!.core
            .invoke("init_site_template", { targetDir: target, template: "thinkphp" })
            .then(() => ({ ok: true }))
            .catch((e: unknown) => ({ ok: false, error: String(e) })),
          dir,
        );
      } catch (e) {
        r = { ok: false, error: String(e) };
      }
      if (!r.ok) {
        // composer/网络异常时给清晰信号但不让整个套件挂掉
        console.log("ThinkPHP install error:", r.error);
      }
      expect(r.ok).toBe(true);
    });

    it("ThinkPHP 入口文件齐", () => {
      // ThinkPHP 6/8 项目骨架：think 命令、composer.json、app/ 目录、vendor/
      expect(fs.existsSync(path.join(dir, "think"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "composer.json"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "vendor"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "app"))).toBe(true);
    });
  });

  // ───────────────────────────────────────
  describe.skip("[6] Laravel 模板（实测 composer create-project）", () => {
    const dir = mkEmptyDir("laravel");

    it("init_site_template(laravel) 完成", async function () {
      this.timeout(420_000);
      let r: { ok: boolean; error?: string };
      try {
        r = await browser.execute((target: string) =>
          window.__TAURI__!.core
            .invoke("init_site_template", { targetDir: target, template: "laravel" })
            .then(() => ({ ok: true }))
            .catch((e: unknown) => ({ ok: false, error: String(e) })),
          dir,
        );
      } catch (e) {
        r = { ok: false, error: String(e) };
      }
      if (!r.ok) {
        console.log("Laravel install error:", r.error);
      }
      expect(r.ok).toBe(true);
    });

    it("Laravel 入口文件齐", () => {
      expect(fs.existsSync(path.join(dir, "artisan"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "composer.json"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "vendor"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "app"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "public", "index.php"))).toBe(true);
    });
  });

  // ───────────────────────────────────────
  describe("[3] 目录非空守卫", () => {
    it("非空目录被后端拒绝", async () => {
      const dir = mkEmptyDir("nonempty");
      fs.writeFileSync(path.join(dir, "preexisting.txt"), "hello");

      // wdio 9 在 browser.execute 里 async + try/catch 对 invoke reject 不可靠，
      // 改用 .then/.catch 链显式包装 + 在外层 try/catch 兜底。
      let r: { ok: boolean; error?: string };
      try {
        r = await browser.execute((target: string) =>
          window.__TAURI__!.core
            .invoke("init_site_template", { targetDir: target, template: "blank" })
            .then(() => ({ ok: true }))
            .catch((e: unknown) => ({ ok: false, error: String(e) })),
          dir,
        );
      } catch (e) {
        r = { ok: false, error: String(e) };
      }

      expect(r.ok).toBe(false);
      expect(r.error || "").toContain("目录非空");
      expect(fs.existsSync(path.join(dir, "preexisting.txt"))).toBe(true);
      expect(fs.existsSync(path.join(dir, "index.php"))).toBe(false);
    });
  });

  // ───────────────────────────────────────
  describe("[4] 不存在目录守卫", () => {
    it("目标目录不存在时返回错误", async () => {
      const ghost = path.join(TMP_ROOT, "no-such-dir-" + Date.now());
      let r: { ok: boolean; error?: string };
      try {
        r = await browser.execute((target: string) =>
          window.__TAURI__!.core
            .invoke("init_site_template", { targetDir: target, template: "blank" })
            .then(() => ({ ok: true }))
            .catch((e: unknown) => ({ ok: false, error: String(e) })),
          ghost,
        );
      } catch (e) {
        r = { ok: false, error: String(e) };
      }

      expect(r.ok).toBe(false);
      expect(r.error || "").toContain("目录不存在");
    });
  });
});
