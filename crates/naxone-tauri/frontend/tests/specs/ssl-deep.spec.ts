/**
 * SSL 证书深度验证：
 * - 证书 + 私钥文件真的写到 ~/.naxone/certs/
 * - PEM 内容合法（含 BEGIN/END CERTIFICATE）
 * - 证书 SAN 含 server_name（用 certutil -dump 解析）
 * - 私钥文件 ACL 收紧（icacls 输出不含 Everyone / Users）
 * - 启用 SSL 的 vhost 生成的 nginx conf 含 listen 443 ssl + ssl_certificate
 */
import { browser } from "@wdio/globals";
import { expect } from "@wdio/globals";
import * as fs from "node:fs";
import * as path from "node:path";
import { execSync } from "node:child_process";
import { waitForApp, navigate } from "../helpers.js";

const HOME = process.env.USERPROFILE || process.env.HOME || "";
const CERTS_DIR = path.join(HOME, ".naxone", "certs");

const TEST_HOST = `ssl-test-${Date.now().toString(36)}.local`;
const TEST_PORT = 8889;

describe("SSL 证书：磁盘文件 + PEM 内容 + ACL", () => {
  let certPath = "";
  let keyPath = "";

  before(async () => {
    await waitForApp();
  });

  it("调 generate_self_signed_cert 拿到证书路径", async () => {
    const result = await browser.execute(async (host: string) => {
      try {
        const cert = await window.__TAURI__!.core.invoke<{ cert_path: string; key_path: string }>(
          "generate_self_signed_cert",
          { serverName: host, aliases: [`*.${host}`] }
        );
        return { ok: true, cert };
      } catch (e) {
        return { ok: false, error: String(e) };
      }
    }, TEST_HOST);
    if (!result.ok) console.error("generate failed:", result.error);
    expect(result.ok).toBe(true);
    certPath = result.cert!.cert_path;
    keyPath = result.cert!.key_path;
  });

  it("证书 + 私钥文件实际写到磁盘（~/.naxone/certs/）", async () => {
    expect(certPath).toContain(CERTS_DIR);
    expect(keyPath).toContain(CERTS_DIR);
    expect(fs.existsSync(certPath)).toBe(true);
    expect(fs.existsSync(keyPath)).toBe(true);
  });

  it("证书 PEM 内容合法（含 BEGIN/END CERTIFICATE）", async () => {
    const certPem = fs.readFileSync(certPath, "utf8");
    expect(certPem).toContain("-----BEGIN CERTIFICATE-----");
    expect(certPem).toContain("-----END CERTIFICATE-----");
    // X509 base64 至少 200 字节
    expect(certPem.length).toBeGreaterThan(500);
  });

  it("私钥 PEM 内容合法（含 BEGIN/END PRIVATE KEY）", async () => {
    const keyPem = fs.readFileSync(keyPath, "utf8");
    // ECDSA 用 EC PRIVATE KEY，RSA 用 RSA PRIVATE KEY 或 PRIVATE KEY
    expect(keyPem).toMatch(/-----BEGIN (?:EC |RSA )?PRIVATE KEY-----/);
    expect(keyPem).toMatch(/-----END (?:EC |RSA )?PRIVATE KEY-----/);
  });

  it("证书 SAN 含 server_name + 通配域名（certutil -dump 解析）", async () => {
    let dump = "";
    try {
      dump = execSync(`certutil -dump "${certPath}"`, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
    } catch (e) {
      console.error("certutil 跑失败:", e);
      throw e;
    }
    // SAN 段应含两个域名
    expect(dump).toContain(TEST_HOST);
    expect(dump).toContain(`*.${TEST_HOST}`);
    // Subject 段含 CN 字段（certutil 输出中 "subject:" 与 "CN=" 在不同行）
    const lower = dump.toLowerCase();
    expect(lower).toContain("subject:");
    expect(lower).toContain("cn=");
  });

  it("私钥文件 ACL 收紧（不含 Everyone / BUILTIN\\Users 通用读权限）", async () => {
    let icaclsOut = "";
    try {
      icaclsOut = execSync(`icacls "${keyPath}"`, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
    } catch (e) {
      console.error("icacls 跑失败:", e);
      throw e;
    }
    // 不应该有 Everyone（中文系统是"所有人"）/ Users
    expect(icaclsOut).not.toMatch(/Everyone:/);
    expect(icaclsOut).not.toMatch(/所有人:/);
    expect(icaclsOut).not.toMatch(/BUILTIN\\Users:/);
    expect(icaclsOut).not.toMatch(/BUILTIN\\用户:/);
    // 应该至少含当前用户名
    const username = process.env.USERNAME || "";
    if (username) {
      expect(icaclsOut).toContain(username);
    }
  });
});

describe("SSL vhost：启用后 nginx conf 含 listen 443 ssl + ssl_certificate", () => {
  let nginxVhostsDir = "";

  before(async () => {
    await waitForApp();
    await navigate("网站");

    // 拿 nginx vhosts 目录路径：通过 services 找 nginx install_path / conf / vhosts
    nginxVhostsDir = await browser.execute(async () => {
      try {
        const services = await window.__TAURI__!.core.invoke<Array<{ kind: string; install_path: string }>>("get_services");
        const ng = services.find((s) => s.kind === "nginx");
        if (!ng) return "";
        // Windows 路径分隔符：用 / 拼，nginx 通常都接受
        return ng.install_path.replace(/\\/g, "/").replace(/\/$/, "") + "/conf/vhosts";
      } catch {
        return "";
      }
    });

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

  it("能定位到 nginx vhosts 目录", async () => {
    if (!nginxVhostsDir) {
      console.warn("没装 Nginx，跳过 SSL vhost 写盘测试");
      return;
    }
    expect(nginxVhostsDir.length).toBeGreaterThan(0);
  });

  it("创建启用 SSL 的 vhost → nginx conf 真的写到磁盘 + 含 listen 443 ssl + ssl_certificate", async () => {
    if (!nginxVhostsDir) return;

    const result = await browser.execute(async (host: string, port: number) => {
      try {
        const cfg = await window.__TAURI__!.core.invoke<{ www_root: string }>("get_config");
        const docRoot = cfg.www_root.replace(/[\\/]+$/, "") + "/" + host;
        const cert = await window.__TAURI__!.core.invoke<{ cert_path: string; key_path: string }>(
          "generate_self_signed_cert",
          { serverName: host, aliases: [] }
        );
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
            ssl_cert: cert.cert_path,
            ssl_key: cert.key_path,
            force_https: true,
            custom_directives: null,
            access_log: null,
            sync_hosts: false,
            expires_at: "",
          },
        });
        return { ok: true };
      } catch (e) {
        return { ok: false, error: String(e) };
      }
    }, TEST_HOST, TEST_PORT);
    if (!result.ok) console.error("create_vhost SSL failed:", result.error);
    expect(result.ok).toBe(true);

    // 找 nginx vhost conf 文件
    const confPath = path.join(nginxVhostsDir.replace(/\//g, path.sep), `${TEST_HOST}_${TEST_PORT}.conf`);
    expect(fs.existsSync(confPath)).toBe(true);

    const conf = fs.readFileSync(confPath, "utf8");
    // 关键 SSL 指令
    expect(conf).toContain("listen        443 ssl");
    expect(conf).toContain("ssl_certificate");
    expect(conf).toContain("ssl_certificate_key");
    // force_https → 应有 301 重定向到 https
    expect(conf).toContain("return 301 https");
    // server_name 写入
    expect(conf).toContain(TEST_HOST);
  });
});
