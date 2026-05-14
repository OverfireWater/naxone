import { reactive } from "vue";

export type ToastLevel = "info" | "success" | "warn" | "error";

export interface ToastItem {
  id: number;
  level: ToastLevel;
  msg: string;
  timer?: number;
}

const state = reactive<{ items: ToastItem[] }>({ items: [] });
let seq = 1;

function push(level: ToastLevel, msg: string, duration = 6000) {
  const id = seq++;
  const item: ToastItem = { id, level, msg };
  state.items.push(item);
  if (duration > 0) {
    item.timer = window.setTimeout(() => dismiss(id), duration);
  }
  return id;
}

function dismiss(id: number) {
  const idx = state.items.findIndex(i => i.id === id);
  if (idx < 0) return;
  const t = state.items[idx].timer;
  if (t) clearTimeout(t);
  state.items.splice(idx, 1);
}

export const toast = {
  info: (m: string, d?: number) => push("info", m, d),
  success: (m: string, d?: number) => push("success", m, d),
  warn: (m: string, d?: number) => push("warn", m, d),
  error: (m: string, d?: number) => push("error", m, d ?? 8000),
  dismiss,
};

/**
 * 把后端原始错误（含 Rust 类型名 / 文件路径 / 嵌套 thiserror）翻成对用户友好的简短中文。
 * 命中已知模式时返回友好版；未命中时只取首行 + 截断长度，避免堆栈刷屏。
 */
export function friendlyError(e: unknown): string {
  const raw = String(e ?? "").trim();
  if (!raw) return "操作失败";

  // nginx [emerg] / apache Syntax error 已经包含 file:line + 失败原因，是最有诊断价值的。
  // 优先级在所有泛化规则之前，避免被 "cannot find" 等吞掉细节。保留原文（截断 400 字符）。
  if (/(nginx:\s*\[emerg\]|nginx:\s*\[alert\]|httpd:\s*Syntax error|configuration file.*test failed)/i.test(raw)) {
    const lines = raw
      .split(/[\r\n]+/)
      .map(l => l.trim())
      .filter(l => /emerg|alert|error|syntax|failed/i.test(l));
    const detail = (lines.length > 0 ? lines.join(" | ") : raw).slice(0, 400);
    return `Nginx/Apache 配置错误: ${detail}`;
  }

  // 优先识别用户能采取行动的常见情况
  if (/(端口|port).{0,5}(被占|占用|in use)/i.test(raw)) {
    const m = raw.match(/(?:端口|port)\s*(\d{2,5})/i);
    return m ? `端口 ${m[1]} 已被占用，请先关闭占用程序或在仪表板"陌生进程"处理` : "端口已被占用";
  }
  if (/(权限不足|permission denied|access is denied|拒绝访问|UAC)/i.test(raw)) {
    return "权限不足，请以管理员身份运行 NaxOne，或在 UAC 弹窗点【是】";
  }
  if (/(已被占用|file locked|被其他进程|sharing violation)/i.test(raw)) {
    return "文件被其它程序占用，请先关闭占用方再重试";
  }
  if (/(timeout|超时|timed out)/i.test(raw)) {
    return "操作超时，请重试或检查后端是否响应";
  }
  if (/(sha256|哈希|hash mismatch|checksum)/i.test(raw)) {
    return "下载文件校验失败（可能被中间人篡改或下载源损坏），已拒绝安装";
  }
  // "找不到"放最后，避免吞掉具体错误。命中时尽量带上含路径的具体那一行。
  if (/(找不到|not found|无法找到|cannot find|file not found)/i.test(raw)) {
    const detailLine = raw.split(/[\r\n]/).find(l =>
      /["'][A-Za-z]:[\\/]/.test(l) || /\.(conf|exe|dll|ini|phar|htaccess|so|cnf|sh|bat)\b/i.test(l)
    );
    if (detailLine) {
      return `文件不存在: ${detailLine.trim().slice(0, 300)}`;
    }
    const firstLine = raw.split(/[\r\n]/)[0].trim();
    return firstLine.length > 240 ? firstLine.slice(0, 240) + "…" : firstLine;
  }

  // 兜底：只取第一行 + 截断
  const firstLine = raw.split(/[\r\n]/)[0].trim();
  return firstLine.length > 240 ? firstLine.slice(0, 240) + "…" : firstLine;
}

export function useToast() {
  return { state, toast };
}
