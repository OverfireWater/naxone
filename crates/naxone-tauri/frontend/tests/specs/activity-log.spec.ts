/**
 * v0.5.12 操作日志规范验证
 *
 * 锁定 CLAUDE.md「操作日志规范」：所有用户主动写操作必须 push_log 到活动日志。
 */
import { browser, expect } from "@wdio/globals";
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

interface LogEntry {
  id: number;
  timestamp: string;
  level: string;
  category: string;
  message: string;
  details: string | null;
  context: unknown;
}

const TS = Date.now().toString(36);

async function safeInvoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<{ ok: true; result: T } | { ok: false; error: string }> {
  // wdio 9 对 promise reject 处理不可靠：execute 内部用 then/catch + 外层 try/catch 双保险
  try {
    const r = await browser.execute(
      (c: string, a: Record<string, unknown> | undefined) =>
        window.__TAURI__!.core
          .invoke(c, a)
          .then((res: unknown) => ({ ok: true, result: res }))
          .catch((e: unknown) => ({ ok: false, error: String(e) })),
      cmd,
      args ?? {},
    );
    return r as { ok: true; result: T } | { ok: false; error: string };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

async function getLogs(category: string, limit = 30): Promise<LogEntry[]> {
  const r = await safeInvoke<LogEntry[]>("get_logs", { category, limit });
  return r.ok ? r.result : [];
}

describe("v0.5.12 操作日志规范", () => {
  before(async () => {
    await waitForApp();
    await browser.setTimeout({ script: 60_000 });
    await navigate("仪表板");
  });

  it("init_site_template(blank) 成功 → site-template 类目 Info 开始 + Success 完成 + details 含 stdout", async () => {
    const dir = path.join(os.tmpdir(), `naxone-log-success-${TS}`);
    if (fs.existsSync(dir)) fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });

    const r = await safeInvoke("init_site_template", { targetDir: dir, template: "blank" });
    expect(r.ok).toBe(true);

    // push_log 是异步写 ring buffer，给点时间
    await browser.pause(300);
    const logs = await getLogs("site-template", 30);
    expect(logs.length).toBeGreaterThanOrEqual(2);

    const infoLog = logs.find((l) => l.level === "info" && l.message.includes("开始"));
    expect(infoLog).toBeTruthy();

    const successLog = logs.find((l) => l.level === "success" && l.message.includes("完成"));
    expect(successLog).toBeTruthy();
    expect(successLog!.details || "").toContain("已创建");
    expect(successLog!.category).toBe("site-template");

    fs.rmSync(dir, { recursive: true, force: true });
  });

  it("init_site_template 目录非空 → site-template 类目 Error + details 含原因", async () => {
    const dir = path.join(os.tmpdir(), `naxone-log-fail-${TS}`);
    if (fs.existsSync(dir)) fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, "block.txt"), "blocker");

    const r = await safeInvoke("init_site_template", { targetDir: dir, template: "blank" });
    expect(r.ok).toBe(false);

    await browser.pause(300);
    const logs = await getLogs("site-template", 30);
    const errLog = logs.find((l) => l.level === "error" && l.message.includes("失败"));
    expect(errLog).toBeTruthy();
    expect(errLog!.details || "").toContain("目录非空");

    fs.rmSync(dir, { recursive: true, force: true });
  });

  it("kill_process_by_pid(0) → port 类目 Error", async () => {
    const r = await safeInvoke("kill_process_by_pid", { pid: 0 });
    expect(r.ok).toBe(false);

    await browser.pause(300);
    const logs = await getLogs("port", 10);
    const errLog = logs.find(
      (l) => l.level === "error" && (l.message.includes("PID 无效") || l.message.includes("结束进程")),
    );
    expect(errLog).toBeTruthy();
  });
});
