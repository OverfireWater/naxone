/**
 * CRUD 冒烟 + HTTPS 链路测试。
 * Tauri v2 + withGlobalTauri=true → window.__TAURI__.core.invoke 可用。
 *
 * 设计要点：所有 invoke 调用 **inline 在 browser.execute 内部**，避免 wdio
 * 跨进程传嵌套对象时的序列化错位（实测 nested req 对象会被某些字段误识别为 sequence）。
 */
import { browser } from "@wdio/globals";
import { expect } from "@wdio/globals";
import { waitForApp, navigate } from "../helpers.js";

const TEST_HOST = `e2e-test-${Date.now().toString(36)}.local`;
const TEST_PORT = 8881;

declare global {
  interface Window {
    __TAURI__?: {
      core: { invoke: <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T> };
    };
  }
}

describe("CRUD 冒烟：创建 / 编辑 / 删除测试站点", () => {
  before(async () => {
    await waitForApp();
    await navigate("网站");
    // 清残留
    await browser.execute(async (host: string, port: number) => {
      try { await window.__TAURI__!.core.invoke("delete_vhost", { id: `${host}_${port}` }); } catch {}
    }, TEST_HOST, TEST_PORT);
  });

  after(async () => {
    await browser.execute(async (host: string, port: number) => {
      try { await window.__TAURI__!.core.invoke("delete_vhost", { id: `${host}_${port}` }); } catch {}
    }, TEST_HOST, TEST_PORT);
  });

  it("__TAURI__ 全局对象注入了（Tauri v2 webview 必备）", async () => {
    const present = await browser.execute(() => !!window.__TAURI__?.core?.invoke);
    expect(present).toBe(true);
  });

  it("能调 create_vhost 创建新站点", async () => {
    const result = await browser.execute(async (host: string, port: number) => {
      try {
        const cfg = await window.__TAURI__!.core.invoke<{ www_root: string }>("get_config");
        const docRoot = cfg.www_root.replace(/[\\/]+$/, "") + "/" + host;
        await window.__TAURI__!.core.invoke("create_vhost", {
          req: {
            server_name: host,
            aliases: "",
            listen_port: port,
            document_root: docRoot,
            php_version: null,
            index_files: "index.php index.html",
            rewrite_rule: "",
            autoindex: false,
            ssl_cert: null,
            ssl_key: null,
            force_https: false,
            custom_directives: null,
            access_log: null,
            sync_hosts: true,
            expires_at: "",
          },
        });
        return { ok: true };
      } catch (e) {
        return { ok: false, error: String(e) };
      }
    }, TEST_HOST, TEST_PORT);
    if (!result.ok) console.error("create_vhost 失败:", result.error);
    expect(result.ok).toBe(true);
  });

  it("vhosts 列表里能看到新站点", async () => {
    const found = await browser.execute(async (host: string) => {
      const list = await window.__TAURI__!.core.invoke<Array<{ server_name: string }>>("get_vhosts");
      return list.some((v) => v.server_name === host);
    }, TEST_HOST);
    expect(found).toBe(true);
  });

  it("update_vhost 启用 SSL（一键生证书 + 写回）", async () => {
    const result = await browser.execute(async (host: string, port: number) => {
      try {
        const cfg = await window.__TAURI__!.core.invoke<{ www_root: string }>("get_config");
        const docRoot = cfg.www_root.replace(/[\\/]+$/, "") + "/" + host;
        const cert = await window.__TAURI__!.core.invoke<{ cert_path: string; key_path: string }>(
          "generate_self_signed_cert",
          { serverName: host, aliases: [] }
        );
        await window.__TAURI__!.core.invoke("update_vhost", {
          id: `${host}_${port}`,
          req: {
            server_name: host,
            aliases: "",
            listen_port: port,
            document_root: docRoot,
            php_version: null,
            index_files: "index.php index.html",
            rewrite_rule: "",
            autoindex: false,
            ssl_cert: cert.cert_path,
            ssl_key: cert.key_path,
            force_https: false,
            custom_directives: null,
            access_log: null,
            sync_hosts: true,
            expires_at: "",
          },
        });
        return { ok: true, cert };
      } catch (e) {
        return { ok: false, error: String(e) };
      }
    }, TEST_HOST, TEST_PORT);
    if (!result.ok) console.error("update_vhost 失败:", result.error);
    expect(result.ok).toBe(true);
    expect(result.cert!.cert_path).toMatch(/\.(crt|pem)$/i);
    expect(result.cert!.key_path).toMatch(/\.key$/i);
  });

  it("启用 SSL 后 vhost 的 has_ssl=true", async () => {
    const v = await browser.execute(async (host: string) => {
      const list = await window.__TAURI__!.core.invoke<Array<{ server_name: string; has_ssl: boolean }>>("get_vhosts");
      return list.find((x) => x.server_name === host) ?? null;
    }, TEST_HOST);
    expect(v).toBeTruthy();
    expect(v!.has_ssl).toBe(true);
  });

  it("删除站点 → vhosts 列表清干净", async () => {
    const remaining = await browser.execute(async (host: string, port: number) => {
      try {
        await window.__TAURI__!.core.invoke("delete_vhost", { id: `${host}_${port}` });
      } catch {}
      const list = await window.__TAURI__!.core.invoke<Array<{ server_name: string }>>("get_vhosts");
      return list.find((v) => v.server_name === host) ?? null;
    }, TEST_HOST, TEST_PORT);
    expect(remaining).toBeNull();
  });
});

describe("HTTPS 证书生成", () => {
  it("generate_self_signed_cert 能跑出 .crt + .key 路径", async () => {
    const result = await browser.execute(async (h: string) => {
      try {
        const cert = await window.__TAURI__!.core.invoke<{ cert_path: string; key_path: string }>(
          "generate_self_signed_cert",
          { serverName: h, aliases: [`*.${h}`] }
        );
        return { ok: true, cert };
      } catch (e) {
        return { ok: false, error: String(e) };
      }
    }, `https-only-${Date.now().toString(36)}.local`);
    if (!result.ok) console.error("generate_self_signed_cert 失败:", result.error);
    expect(result.ok).toBe(true);
    expect(result.cert!.cert_path).toMatch(/\.(crt|pem)$/i);
    expect(result.cert!.key_path).toMatch(/\.key$/i);
    expect(result.cert!.cert_path.toLowerCase()).toContain("naxone");
  });
});

describe("安全注入防护（HIGH-1 / HIGH-2 修复验证）", () => {
  // wdio 9 把 webview 内 Tauri invoke reject 直接当协议错误冒泡，绕过 webview 内 .catch；
  // 所以从 spec 层 try/catch wdio 抛出的 WebDriverError，用错误消息匹配。
  it("HIGH-1：恶意 server_name 含 `;` 应被 vhost.validate() 拒绝", async () => {
    let errorMsg = "";
    try {
      await browser.execute(() =>
        window.__TAURI__!.core
          .invoke<{ www_root: string }>("get_config")
          .then((cfg) =>
            window.__TAURI__!.core.invoke("create_vhost", {
              req: {
                server_name: "a;rm",
                aliases: "",
                listen_port: 9999,
                document_root: cfg.www_root + "/should-not-exist",
                php_version: null,
                index_files: "index.php index.html",
                rewrite_rule: "",
                autoindex: false,
                ssl_cert: null,
                ssl_key: null,
                force_https: false,
                custom_directives: null,
                access_log: null,
                sync_hosts: false,
                expires_at: "",
              },
            })
          )
      );
    } catch (e) {
      errorMsg = String(e);
    }
    expect(errorMsg).not.toBe("");
    expect(errorMsg).toMatch(/不合法|格式|invalid/i);
  });

  it("HIGH-2：document_root 指向 C:\\Windows 应被 validate_document_root 拒绝", async () => {
    let errorMsg = "";
    try {
      await browser.execute(() =>
        window.__TAURI__!.core.invoke("create_vhost", {
          req: {
            server_name: "blocked-test.local",
            aliases: "",
            listen_port: 9998,
            document_root: "C:\\Windows\\System32\\naxone-test",
            php_version: null,
            index_files: "index.php index.html",
            rewrite_rule: "",
            autoindex: false,
            ssl_cert: null,
            ssl_key: null,
            force_https: false,
            custom_directives: null,
            access_log: null,
            sync_hosts: false,
            expires_at: "",
          },
        })
      );
    } catch (e) {
      errorMsg = String(e);
    }
    expect(errorMsg).not.toBe("");
    expect(errorMsg).toMatch(/系统路径|不允许|绝对路径/);
  });
});
