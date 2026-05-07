// 用 puppeteer-core 走 CDP 连本机 Edge，对每个 .html 截 fullPage PNG。
// 跑：node docs/diagrams/render.mjs
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// puppeteer-core 装在 frontend/node_modules
const require = createRequire(path.join(__dirname, "../../crates/naxone-tauri/frontend/package.json"));
const puppeteer = require("puppeteer-core");
const EDGE = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";
const PAGES = ["architecture", "service-flow", "vhost-flow"];
const WIDTH = 1200;

if (!existsSync(EDGE)) {
  console.error(`Edge 不在: ${EDGE}`);
  process.exit(1);
}

const browser = await puppeteer.launch({
  executablePath: EDGE,
  headless: "new",
  args: ["--no-sandbox", "--hide-scrollbars"],
});

try {
  for (const name of PAGES) {
    const html = path.join(__dirname, `${name}.html`);
    const out = path.join(__dirname, `${name}.png`);
    const page = await browser.newPage();
    await page.setViewport({ width: WIDTH, height: 800, deviceScaleFactor: 1 });
    await page.goto(pathToFileURL(html).href, { waitUntil: "networkidle0" });
    await page.screenshot({ path: out, fullPage: true });
    const dim = await page.evaluate(() => ({
      w: document.documentElement.scrollWidth,
      h: document.documentElement.scrollHeight,
    }));
    console.log(`✓ ${name}.png  ${dim.w} x ${dim.h}`);
    await page.close();
  }
} finally {
  await browser.close();
}
