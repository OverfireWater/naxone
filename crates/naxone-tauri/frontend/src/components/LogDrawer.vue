<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { X, Trash2, FolderOpen, RefreshCw, AlertCircle, AlertTriangle, CheckCircle2, Info, Bug, Copy } from "lucide-vue-next";
import { toast } from "../composables/useToast";
import SelectMenu from "./SelectMenu.vue";

interface LogEntry {
  id: number;
  timestamp: string;
  level: "debug" | "info" | "warn" | "error" | "success";
  category: string;
  message: string;
  details?: string;
  context?: any;
}

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

const logs = ref<LogEntry[]>([]);
const levelFilter = ref<string>("");
const categoryFilter = ref<string>("");
const expandedIds = ref<Set<number>>(new Set());

const levelOptions = [
  { label: "全部级别", value: "" },
  { label: "调试+", value: "debug" },
  { label: "信息+", value: "info" },
  { label: "成功+", value: "success" },
  { label: "警告+", value: "warn" },
  { label: "错误", value: "error" },
];
const categoryOptions = [
  { label: "全部分类", value: "" },
  { label: "服务", value: "service" },
  { label: "站点", value: "vhost" },
  { label: "配置", value: "config" },
  { label: "扩展", value: "extension" },
  { label: "设置", value: "settings" },
  { label: "系统", value: "system" },
];

let timer: number | null = null;
let lastId = 0;
const INVOKE_TIMEOUT_MS = 10000;

async function invokeWithTimeout<T>(
  command: string,
  args?: Record<string, unknown>,
  timeoutMs = INVOKE_TIMEOUT_MS,
): Promise<T> {
  const timeout = new Promise<never>((_, reject) => {
    window.setTimeout(() => reject(new Error(`${command} 超时`)), timeoutMs);
  });
  return await Promise.race([invoke<T>(command, args), timeout]);
}

async function loadLogs(fresh = false) {
  try {
    const since = fresh ? undefined : lastId;
    const fetched: LogEntry[] = await invokeWithTimeout("get_logs", {
      limit: 200,
      level: levelFilter.value || null,
      category: categoryFilter.value || null,
      sinceId: since,
    });
    if (fresh) {
      logs.value = fetched;
    } else if (fetched.length > 0) {
      logs.value = [...fetched, ...logs.value].slice(0, 500);
    }
    if (fetched.length > 0) {
      lastId = Math.max(lastId, fetched[0].id);
    }
  } catch (e) {
    toast.warn(`日志读取失败: ${e}`);
  }
}

async function clearLogs() {
  try {
    await invokeWithTimeout("clear_logs");
    logs.value = [];
    lastId = 0;
  } catch (e) {
    toast.error(`清空日志失败: ${e}`);
  }
}

async function openLogDir() {
  try {
    await invokeWithTimeout("open_log_dir");
  } catch (e) {
    toast.error(`打开日志目录失败: ${e}`);
  }
}

function toggleExpand(id: number) {
  if (expandedIds.value.has(id)) expandedIds.value.delete(id);
  else expandedIds.value.add(id);
}

async function copyLog(log: LogEntry) {
  const text = log.details
    ? `[${log.timestamp}] [${log.level.toUpperCase()}] ${log.message}\n${log.details}`
    : `[${log.timestamp}] [${log.level.toUpperCase()}] ${log.message}`;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
  }
  toast.success("已复制到剪贴板");
}

const levelIcon = computed(() => ({
  debug: Bug,
  info: Info,
  success: CheckCircle2,
  warn: AlertTriangle,
  error: AlertCircle,
} as const));

function levelColor(level: string): string {
  return ({
    debug: "var(--text-muted)",
    info: "var(--color-blue-light)",
    success: "var(--color-success-light)",
    warn: "#eab308",
    error: "var(--color-danger)",
  } as Record<string, string>)[level] || "var(--text-muted)";
}

function categoryLabel(cat: string): string {
  return ({ service: "服务", vhost: "站点", config: "配置", extension: "扩展", settings: "设置", system: "系统" } as Record<string, string>)[cat] || cat;
}

watch([levelFilter, categoryFilter], () => {
  lastId = 0;
  loadLogs(true);
});

watch(() => props.open, (v) => {
  if (v) {
    loadLogs(true);
    timer = window.setInterval(() => loadLogs(false), 2000);
  } else {
    if (timer) { clearInterval(timer); timer = null; }
  }
});

onMounted(() => {
  if (props.open) loadLogs(true);
});

onUnmounted(() => { if (timer) clearInterval(timer); });
</script>

<template>
  <Transition name="drawer">
    <div v-if="open" class="fixed inset-0 z-[90]">
      <div class="absolute inset-0" style="background: rgba(0,0,0,0.3)" @click="emit('close')"></div>
      <div class="absolute top-0 right-0 h-full flex flex-col shadow-2xl"
           @click.stop
           style="width: 480px; background: var(--bg-secondary); border-left: 1px solid var(--border-color)">
        <div class="flex items-center justify-between px-4 py-3 border-b" style="border-color: var(--border-color)">
          <div class="flex items-center gap-2">
            <span class="text-[16px] font-semibold">活动日志</span>
            <span class="text-[13px]" style="color: var(--text-muted)">{{ logs.length }} 条</span>
          </div>
          <button class="p-1 rounded hover:bg-[var(--bg-hover)] transition-colors" @click="emit('close')">
            <X :size="16" />
          </button>
        </div>

        <div class="flex items-center gap-2 px-4 py-2 border-b" style="border-color: var(--border-color)">
          <SelectMenu v-model="levelFilter" :options="levelOptions" trigger-class="input" />
          <SelectMenu v-model="categoryFilter" :options="categoryOptions" trigger-class="input" />
          <button class="btn btn-secondary btn-sm !px-2" @click="loadLogs(true)" title="刷新"><RefreshCw :size="13" /></button>
          <button class="btn btn-secondary btn-sm !px-2" @click="openLogDir" title="打开日志目录"><FolderOpen :size="13" /></button>
          <button class="btn btn-secondary btn-sm !px-2" @click="clearLogs" title="清空"><Trash2 :size="13" /></button>
        </div>

        <div class="flex-1 overflow-y-auto">
          <div v-if="logs.length === 0" class="text-center py-12 text-[16px]" style="color: var(--text-muted)">暂无日志</div>
          <div v-for="log in logs" :key="log.id"
               class="px-4 py-2 border-b cursor-pointer transition-colors hover:bg-[var(--bg-hover)] group"
               :style="{ borderColor: 'var(--border-color)' }"
               @click="toggleExpand(log.id)">
            <div class="flex items-start gap-2.5">
              <component :is="levelIcon[log.level]" :size="14" :style="{ color: levelColor(log.level), marginTop: '2px' }" class="shrink-0" />
              <div class="flex-1 min-w-0">
                <div class="flex items-baseline gap-2">
                  <span class="text-[13px] font-mono" style="color: var(--text-secondary)">{{ log.timestamp.slice(11, 19) }}</span>
                  <span class="text-[13px] px-2 py-0.5 rounded font-medium" style="background: var(--bg-tertiary); color: var(--text-secondary); border: 1px solid var(--border-color)">{{ categoryLabel(log.category) }}</span>
                </div>
                <div class="text-[16px] mt-0.5 flex items-start justify-between gap-2">
                  <span style="color: var(--text-primary)">{{ log.message }}</span>
                  <button class="shrink-0 p-1 rounded opacity-0 group-hover:opacity-60 hover:!opacity-100 transition-opacity"
                          style="color: var(--text-muted); background: transparent; border: none; cursor: pointer"
                          title="复制"
                          @click.stop="copyLog(log)">
                    <Copy :size="12" />
                  </button>
                </div>
                <pre v-if="expandedIds.has(log.id) && log.details" class="text-[13px] mt-2 p-2.5 rounded whitespace-pre-wrap font-mono"
                     style="background: var(--bg-input); color: var(--text-primary); border: 1px solid var(--border-color); max-height: 200px; overflow-y: auto">{{ log.details }}</pre>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.drawer-enter-active, .drawer-leave-active { transition: opacity 0.2s ease; }
.drawer-enter-active .absolute.top-0, .drawer-leave-active .absolute.top-0 { transition: transform 0.25s cubic-bezier(0.4, 0, 0.2, 1); }
.drawer-enter-from, .drawer-leave-to { opacity: 0; }
.drawer-enter-from .absolute.top-0, .drawer-leave-to .absolute.top-0 { transform: translateX(100%); }
</style>
