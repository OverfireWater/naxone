/**
 * v0.5.8 完整性测试
 *
 * 覆盖：mkcert SSL CA、端口诊断、PIE 扩展、Raw conf 读写、SSL 端口列显示。
 * 每个用例除了调 invoke 看返回值，还会 verify **文件系统副作用**（CA 文件 / conf 真改了 / stamp 真写了）。
 */
import { browser } from "@wdio/globals";
import { expect } from "@wdio/globals";
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

const HOME = process.env.USERPROFILE || os.homedir();
const CERT_DIR = path.join(HOME, ".naxone", "certs");
const CA_DIR = path.join(CERT_DIR, "_ca");
const CA_CRT = path.join(CA_DIR, "naxone-rootCA.crt");
const CA_KEY = path.join(CA_DIR, "naxone-rootCA.key");
const CA_INSTALLED_STAMP = path.join(CA_DIR, ".installed");

describe("v0.5.8 完整性测试", () => {
  before(async () => {
    await waitForApp();
  });

  // ─────────────────────────────────────────────────────────────
  describe("[1] mkcert：本地 CA + leaf 证书", () => {
    const host = `e2e-mkcert-${Date.now().toString(36)}.local`;
    let leafCert: string | null = null;
    let leafKey: string | null = null;

    it("generate_self_signed_cert 返回真实路径", async () => {
      const r = await browser.execute(async (h: string) => {
        try {
          const res = await window.__TAURI__!.core.invoke<{ cert_path: string; key_path: string }>(
            "generate_self_signed_cert",
            { serverName: h, aliases: [] },
          );
          return { ok: true, ...res };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      }, host);
      expect(r.ok).toBe(true);
      expect(r.cert_path).toBeTruthy();
      expect(r.key_path).toBeTruthy();
      leafCert = r.cert_path!;
      leafKey = r.key_path!;
    });

    it("leaf 证书文件真写到磁盘", () => {
      expect(fs.existsSync(leafCert!)).toBe(true);
      expect(fs.existsSync(leafKey!)).toBe(true);
      const pem = fs.readFileSync(leafCert!, "utf8");
      expect(pem).toContain("-----BEGIN CERTIFICATE-----");
    });

    it("CA 证书 + 私钥被生成", () => {
      expect(fs.existsSync(CA_CRT)).toBe(true);
      expect(fs.existsSync(CA_KEY)).toBe(true);
      const caPem = fs.readFileSync(CA_CRT, "utf8");
      expect(caPem).toContain("-----BEGIN CERTIFICATE-----");
    });

    it("CA 信任库安装 stamp 写入 (.installed)", () => {
      expect(fs.existsSync(CA_INSTALLED_STAMP)).toBe(true);
    });

    it("二次签发复用同一 CA（CA 证书没变）", async () => {
      const before = fs.readFileSync(CA_CRT, "utf8");
      await browser.execute(async (h: string) => {
        await window.__TAURI__!.core.invoke("generate_self_signed_cert", {
          serverName: `${h}-second`,
          aliases: [],
        });
      }, host);
      const after = fs.readFileSync(CA_CRT, "utf8");
      expect(after).toBe(before);
    });
  });

  // ─────────────────────────────────────────────────────────────
  describe("[2] 端口诊断", () => {
    it("diagnose_port(80) 返回真实占用进程信息", async () => {
      const r = await browser.execute(async () => {
        try {
          const res = await window.__TAURI__!.core.invoke<{
            in_use: boolean;
            listeners: Array<{ pid: number; process_name: string | null; exe_path: string | null }>;
          }>("diagnose_port", { port: 80 });
          return { ok: true, ...res };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      });
      expect(r.ok).toBe(true);
      expect(r.in_use).toBe(true);
      expect(r.listeners!.length).toBeGreaterThan(0);
      const first = r.listeners![0];
      expect(first.pid).toBeGreaterThan(0);
      expect(first.process_name).toBeTruthy();
    });

    it("diagnose_port(54329) 一个不太可能被占的端口显示空闲", async () => {
      const r = await browser.execute(async () => {
        return await window.__TAURI__!.core.invoke<{ in_use: boolean }>("diagnose_port", { port: 54329 });
      });
      expect(r.in_use).toBe(false);
    });

    // 备注：diagnose_port(0) 边界测试在 webdriver 下被驱动层吞 reject，
    // 改在 cargo test 单测覆盖（commands/port.rs），此处不再 e2e。
  });

  // ─────────────────────────────────────────────────────────────
  describe("[3] PIE 扩展", () => {
    it("pie_runtime_info 找到 PHP ≥ 8.1", async () => {
      const r = await browser.execute(async () => {
        return await window.__TAURI__!.core.invoke<{
          runtime_php_path: string | null;
          runtime_version: string | null;
        }>("pie_runtime_info");
      });
      expect(r.runtime_php_path).toBeTruthy();
      expect(r.runtime_version).toBeTruthy();
      // 验证版本 ≥ 8.1
      const [major, minor] = r.runtime_version!.split(".").map(Number);
      expect(major === 8 ? minor >= 1 : major > 8).toBe(true);
    });

    it("pie_search('redis') 返回非空 + 含 redis 相关包", async () => {
      const r = await browser.execute(async () => {
        try {
          const res = await window.__TAURI__!.core.invoke<Array<{ name: string; description: string }>>(
            "pie_search",
            { keyword: "redis" },
          );
          return { ok: true, count: res.length, names: res.map((x) => x.name) };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      });
      if (!r.ok) console.error("pie_search redis 失败:", r.error);
      expect(r.ok).toBe(true);
      expect(r.count).toBeGreaterThan(0);
      expect(r.names!.some((n) => n.toLowerCase().includes("redis"))).toBe(true);
    });

    it("pie_search('') 默认列出热门 php-ext", async () => {
      const r = await browser.execute(async () => {
        try {
          const res = await window.__TAURI__!.core.invoke<Array<{ name: string }>>("pie_search", { keyword: "" });
          return { ok: true, count: res.length };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      });
      expect(r.ok).toBe(true);
      // packagist 应该返回 30 条（per_page）
      expect(r.count).toBeGreaterThan(0);
    });
  });

  // ─────────────────────────────────────────────────────────────
  describe("[4] Raw nginx conf 读写", () => {
    let vhostId: string | null = null;
    let originalContent: string | null = null;

    it("get_vhosts 至少有一个站点用来测试", async () => {
      const id = await browser.execute(async () => {
        const list = await window.__TAURI__!.core.invoke<Array<{ id: string }>>("get_vhosts");
        return list[0]?.id || null;
      });
      expect(id).toBeTruthy();
      vhostId = id;
    });

    it("read_vhost_conf 返回真实文件路径 + 内容含 server_name", async () => {
      const r = await browser.execute(async (id: string) => {
        try {
          const res = await window.__TAURI__!.core.invoke<{ path: string; content: string }>(
            "read_vhost_conf",
            { id },
          );
          return { ok: true, ...res };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      }, vhostId!);
      expect(r.ok).toBe(true);
      expect(r.path!).toMatch(/\.conf$/);
      expect(fs.existsSync(r.path!)).toBe(true);
      expect(r.content!).toContain("server_name");
      originalContent = r.content!;
    });

    it("write_vhost_conf 写入后磁盘真变化", async () => {
      const marker = `# e2e-marker-${Date.now()}`;
      const newContent = originalContent! + "\n" + marker + "\n";
      const r = await browser.execute(async (id: string, content: string, mk: string) => {
        try {
          await window.__TAURI__!.core.invoke("write_vhost_conf", { id, content });
          const after = await window.__TAURI__!.core.invoke<{ content: string }>("read_vhost_conf", { id });
          return { ok: true, hasMarker: after.content.includes(mk) };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      }, vhostId!, newContent, marker);
      expect(r.ok).toBe(true);
      expect(r.hasMarker).toBe(true);
    });

    it("还原 conf（清理副作用）", async () => {
      const r = await browser.execute(async (id: string, content: string) => {
        try {
          await window.__TAURI__!.core.invoke("write_vhost_conf", { id, content });
          return { ok: true };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      }, vhostId!, originalContent!);
      expect(r.ok).toBe(true);
    });
  });

  // ─────────────────────────────────────────────────────────────
  describe("[5] SSL 端口列显示 80/443", () => {
    const sslHost = `e2e-ssl-${Date.now().toString(36)}.local`;
    const sslPort = 8884;

    after(async () => {
      await browser.execute(
        async (h: string, p: number) => {
          try {
            await window.__TAURI__!.core.invoke("delete_vhost", { id: `${h}_${p}` });
          } catch {}
        },
        sslHost,
        sslPort,
      );
    });

    it("创建启用 SSL 的临时 vhost", async () => {
      const r = await browser.execute(
        async (h: string, p: number) => {
          try {
            const cfg = await window.__TAURI__!.core.invoke<{ www_root: string }>("get_config");
            const docRoot = cfg.www_root.replace(/[\\/]+$/, "") + "/" + h;
            const cert = await window.__TAURI__!.core.invoke<{ cert_path: string; key_path: string }>(
              "generate_self_signed_cert",
              { serverName: h, aliases: [] },
            );
            await window.__TAURI__!.core.invoke("create_vhost", {
              req: {
                server_name: h,
                aliases: "",
                listen_port: p,
                document_root: docRoot,
                php_version: null,
                index_files: "index.php",
                rewrite_rule: "",
                autoindex: false,
                ssl_cert: cert.cert_path,
                ssl_key: cert.key_path,
                force_https: false,
                custom_directives: null,
                access_log: null,
                sync_hosts: false,
                expires_at: "",
              },
            });
            const list = await window.__TAURI__!.core.invoke<Array<any>>("get_vhosts");
            const v = list.find((x) => x.server_name === h);
            return { ok: true, has_ssl: v?.has_ssl, listen_port: v?.listen_port };
          } catch (e) {
            return { ok: false, error: String(e) };
          }
        },
        sslHost,
        sslPort,
      );
      if (!r.ok) console.error("create ssl vhost 失败:", r.error);
      expect(r.ok).toBe(true);
      expect(r.has_ssl).toBe(true);
      expect(r.listen_port).toBe(sslPort);
    });

    it("生成的 nginx conf 真含 'listen 443 ssl'（SSL 真生效）", async () => {
      // 这才是 SSL 真生效的证明：UI 文案只是装饰，nginx conf 才决定行为
      const r = await browser.execute(async (h: string, p: number) => {
        try {
          const res = await window.__TAURI__!.core.invoke<{ path: string; content: string }>(
            "read_vhost_conf",
            { id: `${h}_${p}` },
          );
          return { ok: true, ...res };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      }, sslHost, sslPort);
      expect(r.ok).toBe(true);
      expect(fs.existsSync(r.path!)).toBe(true);
      expect(r.content!).toMatch(/listen\s+443\s+ssl/);
      expect(r.content!).toContain("ssl_certificate");
      expect(r.content!).toContain("ssl_certificate_key");
    });
  });
});
