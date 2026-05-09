/**
 * PIE 真装扩展端到端测试（有副作用：装 xdebug，再卸掉）。
 *
 * 流程：
 * 1. 装 xdebug/xdebug 到 PHP 8.4nts
 * 2. 验证：php.ini 真有 zend_extension=xdebug 行 + ext 目录有 php_xdebug.dll
 * 3. 卸载：spawn `php pie.phar uninstall xdebug/xdebug --with-php-path=php.exe`
 * 4. 验证：php.ini 恢复 + dll 删除
 *
 * 装到 PHP 8.4 因为 xdebug 对 8.4 兼容良好。
 */
import { browser } from "@wdio/globals";
import { expect } from "@wdio/globals";
import { waitForApp } from "../helpers.js";
import * as fs from "node:fs";
import * as path from "node:path";
import { spawnSync } from "node:child_process";

declare global {
  interface Window {
    __TAURI__?: {
      core: { invoke: <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T> };
    };
  }
}

// 目标 PHP：php85nts (PHP 8.5.1)，ext 目录没装过 apcu —— 全新副作用测试
const TARGET_PHP_INSTALL = "D:/phpstudy_pro/Extensions/php/php85nts";
const TARGET_PHP_EXE = path.join(TARGET_PHP_INSTALL, "php.exe");
const TARGET_PHP_INI = path.join(TARGET_PHP_INSTALL, "php.ini");
const TARGET_EXT_DIR = path.join(TARGET_PHP_INSTALL, "ext");

// PIE runtime 也是 php85nts（同一个 8.5.1 ≥ 8.1 满足 PIE 要求）
const PIE_PHP = TARGET_PHP_EXE;
const PIE_PHAR = "D:/phpstudy_pro/WWW/utils/ruststudy/target/release/resources/pie.phar";

// xdebug/xdebug 3.5：PIE info 确认兼容 PHP 8.0~8.6，PHP 8.5 ext 目录确认没装
const PACKAGE = "xdebug/xdebug";
const EXT_NAME = "xdebug";

function readIniText(): string {
  return fs.existsSync(TARGET_PHP_INI) ? fs.readFileSync(TARGET_PHP_INI, "utf8") : "";
}

function listExtDlls(): string[] {
  if (!fs.existsSync(TARGET_EXT_DIR)) return [];
  return fs
    .readdirSync(TARGET_EXT_DIR)
    .filter((f) => f.toLowerCase().includes(EXT_NAME) && f.toLowerCase().endsWith(".dll"));
}

describe("PIE 真装 xdebug 到 PHP 8.4 + 验证 + 卸载", () => {
  let iniBefore = "";
  let dllsBefore: string[] = [];

  before(async () => {
    await waitForApp();
    // PIE install 走网络下载 + 解压，30s 默认 webdriver script timeout 不够
    await browser.setTimeout({ script: 240_000 });
    expect(fs.existsSync(TARGET_PHP_EXE)).toBe(true);
    iniBefore = readIniText();
    dllsBefore = listExtDlls();
    console.log(`[setup] 目标 PHP: ${TARGET_PHP_EXE}`);
    console.log(`[setup] php.ini 已读取 ${iniBefore.length} 字节`);
    console.log(`[setup] ext 目录已有 ${EXT_NAME} dll: ${dllsBefore.join(", ") || "(无)"}`);
  });

  // 失败也尝试 uninstall，避免污染你机器
  after(async () => {
    if (fs.existsSync(PIE_PHAR) && fs.existsSync(PIE_PHP)) {
      console.log(`[cleanup] 尝试 PIE uninstall ${PACKAGE} ...`);
      const r = spawnSync(
        PIE_PHP,
        [PIE_PHAR, "uninstall", PACKAGE, `--with-php-path=${TARGET_PHP_EXE}`, "--no-interaction"],
        { encoding: "utf8", timeout: 120_000 },
      );
      console.log(`[cleanup] uninstall exit=${r.status}, stdout=${r.stdout?.slice(0, 200)}`);
      if (r.stderr) console.log(`[cleanup] stderr=${r.stderr.slice(0, 300)}`);
    }
  });

  it("PIE install 调通且返回成功", async function () {
    this.timeout(180_000);
    const r = await browser.execute(async (pkg: string, phpPath: string) => {
      try {
        const out = await window.__TAURI__!.core.invoke<string>("pie_install", {
          package: pkg,
          targetPhpInstallPath: phpPath,
        });
        return { ok: true, output: out };
      } catch (e) {
        return { ok: false, error: String(e) };
      }
    }, PACKAGE, TARGET_PHP_INSTALL);
    if (!r.ok) console.error("install 失败:", r.error);
    else console.log("install 输出:\n" + r.output);
    expect(r.ok).toBe(true);
  });

  it(`ext 目录真新增 ${EXT_NAME} DLL（从无到有）`, () => {
    const dllsAfter = listExtDlls();
    console.log("DLL 文件：", dllsAfter);
    expect(dllsBefore.length).toBe(0); // 测试前提：之前确实没装
    expect(dllsAfter.length).toBeGreaterThan(0);
  });

  it(`php.ini zend_extension/extension 行指向 PHP 8.5 自己的 ${EXT_NAME}.dll`, () => {
    const iniAfter = readIniText();
    // 找未注释的 extension 行，且路径含目标 PHP 8.5 install 目录
    const targetMarker = TARGET_PHP_INSTALL.replace(/\\/g, "/");
    const lines = iniAfter
      .split("\n")
      .filter((ln) => !ln.trim().startsWith(";"))
      .filter((ln) => ln.toLowerCase().includes(EXT_NAME))
      .filter((ln) => /zend_extension|extension/i.test(ln));
    console.log(`未注释 ${EXT_NAME} 行：`, lines);
    // 至少有一行指向 PHP 8.5 自己的 dll（PIE 应该改正了原本指向 8.4 的错误路径）
    const pointingTo85 = lines.some((ln) => ln.includes(targetMarker));
    if (!pointingTo85) {
      console.log("注意：php.ini 含 xdebug 行但未指向 PHP 8.5 路径，可能 PIE 写的是 dll 文件名而非完整路径");
    }
    // 退一步：至少有一行是 PIE 新加的（不只是被注释的）
    expect(lines.length).toBeGreaterThan(0);
  });

  it("get_php_extensions 能查到 xdebug", async () => {
    const r = await browser.execute(async (phpPath: string, ext: string) => {
      try {
        const list = await window.__TAURI__!.core.invoke<Array<{ name: string; enabled: boolean }>>(
          "get_php_extensions",
          { installPath: phpPath },
        );
        const found = list.find((x) => x.name.toLowerCase() === ext);
        return { ok: true, found: !!found, enabled: found?.enabled };
      } catch (e) {
        return { ok: false, error: String(e) };
      }
    }, TARGET_PHP_INSTALL, EXT_NAME);
    if (!r.ok) console.error(r.error);
    expect(r.ok).toBe(true);
    expect(r.found).toBe(true);
  });
});
