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

export function useToast() {
  return { state, toast };
}
