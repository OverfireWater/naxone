<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { X, Trash2, FolderOpen, RefreshCw, AlertCircle, AlertTriangle, CheckCircle2, Info, Bug } from "lucide-vue-next";

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
const levelFilter = ref<string>("");       // "", "warn", "error", "info", "debug"
const categoryFilter = ref<string>("");    // "", "service", "vhost", ...
const expandedIds = ref<Set<number>>(new Set());

let timer: number | null = null;
let lastId = 0;

async function loadLogs(fresh = false) {
  try {
    const since = fresh ? undefined : lastId;
    const fetched: LogEntry[] = await invoke("get_logs", {
      limit: 200,
      level: levelFilter.value || null,
      category: categoryFilter.value || null,
      sinceId: since,
    });
    if (fresh) {
      logs.value = fetched;
    } else if (fetched.length > 0) {
      // Prepend new entries
      logs.value = [...fetched, ...logs.value].slice(0, 500);
    }
    if (fetched.length > 0) {
      lastId = Math.max(lastId, fetched[0].id);
    }
  } catch {}
}

async function clearLogs() {
  await invoke("clear_logs");
  logs.value = [];
  lastId = 0;
}

async function openLogDir() {
  try { await invoke("open_log_dir"); } catch {}
}

function toggleExpand(id: number) {
  if (expandedIds.value.has(id)) expandedIds.value.delete(id);
  else expandedIds.value.add(id);
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

// Reload when filters change
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
    <div v-if="open" class="fixed inset-0 z-[90]" @click.self="emit('close')">
      <div class="absolute inset-0" style="background: rgba(0,0,0,0.3)"></div>
      <div class="absolute top-0 right-0 h-full flex flex-col shadow-2xl"
           style="width: 480px; background: var(--bg-secondary); border-left: 1px solid var(--border-color)">
        <!-- Header -->
        <div class="flex items-center justify-between px-4 py-3 border-b" style="border-color: var(--border-color)">
          <div class="flex items-center gap-2">
            <span class="text-sm font-semibold">活动日志</span>
            <span class="text-xs" style="color: var(--text-muted)">{{ logs.length }} 条</span>
          </div>
          <button class="p-1 rounded hover:bg-[var(--bg-hover)] transition-colors" @click="emit('close')">
            <X :size="16" />
          </button>
        </div>

        <!-- Toolbar -->
        <div class="flex items-center gap-2 px-4 py-2 border-b" style="border-color: var(--border-color)">
          <select class="input sel" style="width: 110px; padding: 4px 24px 4px 8px; font-size: 12px" v-model="levelFilter">
            <option value="">全部级别</option>
            <option value="debug">调试+</option>
            <option value="info">信息+</option>
            <option value="success">成功+</option>
            <option value="warn">警告+</option>
            <option value="error">错误</option>
          </select>
          <select class="input sel" style="width: 110px; padding: 4px 24px 4px 8px; font-size: 12px" v-model="categoryFilter">
            <option value="">全部分类</option>
            <option value="service">服务</option>
            <option value="vhost">站点</option>
            <option value="config">配置</option>
            <option value="extension">扩展</option>
            <option value="settings">设置</option>
            <option value="system">系统</option>
          </select>
          <button class="btn btn-secondary btn-sm !px-2" @click="loadLogs(true)" title="刷新"><RefreshCw :size="13" /></button>
          <button class="btn btn-secondary btn-sm !px-2" @click="openLogDir" title="打开日志目录"><FolderOpen :size="13" /></button>
          <button class="btn btn-secondary btn-sm !px-2" @click="clearLogs" title="清空"><Trash2 :size="13" /></button>
        </div>

        <!-- Log list -->
        <div class="flex-1 overflow-y-auto">
          <div v-if="logs.length === 0" class="text-center py-12 text-[13px]" style="color: var(--text-muted)">暂无日志</div>
          <div v-for="log in logs" :key="log.id"
               class="px-4 py-2 border-b cursor-pointer transition-colors hover:bg-[var(--bg-hover)]"
               :style="{ borderColor: 'var(--border-color)' }"
               @click="toggleExpand(log.id)">
            <div class="flex items-start gap-2.5">
              <component :is="levelIcon[log.level]" :size="14" :style="{ color: levelColor(log.level), marginTop: '2px' }" class="shrink-0" />
              <div class="flex-1 min-w-0">
                <div class="flex items-baseline gap-2">
                  <span class="text-[11px] font-mono" style="color: var(--text-muted)">{{ log.timestamp.slice(11, 19) }}</span>
                  <span class="text-[10px] px-1.5 py-0.5 rounded" style="background: var(--bg-tertiary); color: var(--text-muted)">{{ categoryLabel(log.category) }}</span>
                </div>
                <div class="text-[13px] mt-0.5" style="color: var(--text-primary)">{{ log.message }}</div>
                <pre v-if="expandedIds.has(log.id) && log.details" class="text-[11px] mt-2 p-2 rounded whitespace-pre-wrap font-mono"
                     style="background: var(--bg-primary); color: var(--text-secondary); max-height: 200px; overflow-y: auto">{{ log.details }}</pre>
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
