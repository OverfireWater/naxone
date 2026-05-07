import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { Options } from "@wdio/types";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT = path.resolve(__dirname, "../../..");
const APP = path.join(ROOT, "target", "release", "NaxOne.exe");
const MSEDGEDRIVER = path.join(ROOT, "target", "edgedriver", "msedgedriver.exe");

let tauriDriver: ChildProcessWithoutNullStreams | null = null;

export const config: Options.Testrunner = {
  runner: "local",
  specs: ["./tests/specs/**/*.spec.ts"],
  maxInstances: 1,
  capabilities: [
    {
      maxInstances: 1,
      // Tauri-driver 自动认这个 capability，启动 application 指定的 exe
      "tauri:options": { application: APP },
    } as WebdriverIO.Capabilities,
  ],
  logLevel: "warn",
  bail: 0,
  baseUrl: "tauri://localhost",
  waitforTimeout: 10_000,
  connectionRetryTimeout: 60_000,
  connectionRetryCount: 1,

  // 测试框架
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: 60_000,
  },
  reporters: ["spec"],

  // tauri-driver 监听 4444（默认）。wdio 连这个端口
  hostname: "127.0.0.1",
  port: 4444,

  // 启动前 spawn tauri-driver（指向我们下载的 msedgedriver）；结束后 kill 掉
  onPrepare: () => {
    return new Promise<void>((resolve, reject) => {
      tauriDriver = spawn(
        "tauri-driver",
        ["--native-driver", MSEDGEDRIVER, "--port", "4444"],
        { stdio: ["ignore", "pipe", "pipe"] }
      );
      let resolved = false;
      const onReady = () => {
        if (!resolved) {
          resolved = true;
          resolve();
        }
      };
      // tauri-driver 启动 200ms 后通常就 ready，给 1s 缓冲
      setTimeout(onReady, 1500);
      tauriDriver.on("error", (e) => {
        if (!resolved) {
          resolved = true;
          reject(e);
        }
      });
      tauriDriver.stderr.on("data", (d) => process.stderr.write(`[tauri-driver] ${d}`));
    });
  },

  onComplete: () => {
    if (tauriDriver && !tauriDriver.killed) tauriDriver.kill();
  },
};
