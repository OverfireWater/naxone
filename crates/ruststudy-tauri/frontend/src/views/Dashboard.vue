<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { AlertCircle, AlertTriangle, CheckCircle2, Info, Bug, ChevronRight, Store, Settings2 } from "lucide-vue-next";
import { useRouter } from "vue-router";

const router = useRouter();
import LogDrawer from "../components/LogDrawer.vue";

interface ServiceInfo {
  id: string; kind: string; display_name: string; version: string;
  variant: string | null; port: number;
  status: { state: string; pid?: number }; install_path: string;
  origin: string; // "phpstudy" | "store" | "manual"
}

interface LogEntry {
  id: number; timestamp: string; level: string; category: string;
  message: string; details?: string;
}

const services = ref<ServiceInfo[]>([]);
const loaded = ref(false); // 首次 loadServices 返回后置 true，用于切换 skeleton
import { toast } from "../composables/useToast";
const busyIds = ref<Set<string>>(new Set());
const recentLogs = ref<LogEntry[]>([]);
const logDrawerOpen = ref(false);
const batchBusy = ref(false);

let svcTimer: number | null = null;
let logTimer: number | null = null;
let pauseUntil = 0;

// ==================== Computed ====================

const mainServices = computed(() => {
  const kinds = ["nginx", "apache", "mysql", "redis"];
  return kinds.map(k => services.value.find(s => s.kind === k)).filter(Boolean) as ServiceInfo[];
});

const phpServices = computed(() => services.value.filter(s => s.kind === "php"));

const runningCount = computed(() => services.value.filter(isRunning).length);
const totalCount = computed(() => services.value.length);
const stoppedCount = computed(() => totalCount.value - runningCount.value);
const phpRunning = computed(() => phpServices.value.filter(isRunning).length);

// ==================== Helpers ====================

function isRunning(s: ServiceInfo): boolean { return s.status.state === "Running"; }
function isBusy(id: string): boolean { return busyIds.value.has(id); }
function originBadge(s: ServiceInfo): { text: string; color: string } | null {
  if (s.origin === "phpstudy") return { text: "PS", color: "#8b5cf6" };
  if (s.origin === "manual") return { text: "独立", color: "#06b6d4" };
  if (s.origin === "store") return { text: "商店", color: "#22c55e" };
  return null;
}

function kindLabel(kind: string): string {
  return ({ nginx: "Nginx", apache: "Apache", mysql: "MySQL", redis: "Redis" } as Record<string, string>)[kind] || kind;
}

function showError(msg: string) {
  toast.error(String(msg));
}

function pauseAutoRefresh() { pauseUntil = Date.now() + 5000; }

// ==================== Data loading ====================

async function loadServices(force = false) {
  if (!force && Date.now() < pauseUntil) return;
  try {
    services.value = await invoke("get_services");
    loaded.value = true;
  } catch (e) { showError("加载失败: " + e); }
}

async function loadRecentLogs() {
  try { recentLogs.value = await invoke("get_logs", { limit: 6 }); }
  catch {}
}

async function checkStartupErrors() {
  try {
    const errs: string[] = await invoke("get_startup_errors");
    for (const e of errs) showError(e);
  } catch {}
}

// ==================== Actions ====================

async function startService(id: string) {
  busyIds.value.add(id); pauseAutoRefresh();
  try { services.value = await invoke("start_service", { id }); }
  catch (e) { showError("启动失败: " + e); }
  finally { busyIds.value.delete(id); }
}

async function stopService(id: string) {
  busyIds.value.add(id); pauseAutoRefresh();
  try {
    const updated: ServiceInfo = await invoke("stop_service", { id });
    const idx = services.value.findIndex(s => s.id === id);
    if (idx >= 0) services.value[idx] = updated;
  } catch (e) { showError("停止失败: " + e); }
  finally { busyIds.value.delete(id); }
}

async function restartService(id: string) {
  busyIds.value.add(id); pauseAutoRefresh();
  try {
    const updated: ServiceInfo = await invoke("restart_service", { id });
    const idx = services.value.findIndex(s => s.id === id);
    if (idx >= 0) services.value[idx] = updated;
  } catch (e) { showError("重启失败: " + e); }
  finally { busyIds.value.delete(id); }
}

async function startAll() {
  if (batchBusy.value) return;
  batchBusy.value = true; pauseAutoRefresh();
  try { services.value = await invoke("start_all"); }
  catch (e) { showError("全部启动失败: " + e); }
  finally { batchBusy.value = false; }
}

async function stopAll() {
  if (batchBusy.value) return;
  batchBusy.value = true; pauseAutoRefresh();
  try { services.value = await invoke("stop_all"); }
  catch (e) { showError("全部停止失败: " + e); }
  finally { batchBusy.value = false; }
}

async function startAllPhp() {
  pauseAutoRefresh();
  for (const p of phpServices.value.filter(p => !isRunning(p))) {
    busyIds.value.add(p.id);
    try { services.value = await invoke("start_service", { id: p.id }); } catch {}
    finally { busyIds.value.delete(p.id); }
  }
}

async function stopAllPhp() {
  pauseAutoRefresh();
  for (const p of phpServices.value.filter(isRunning)) {
    busyIds.value.add(p.id);
    try { services.value = await invoke("stop_service", { id: p.id }); } catch {}
    finally { busyIds.value.delete(p.id); }
  }
}

// ==================== Log display ====================

const levelIconMap: Record<string, any> = {
  debug: Bug, info: Info, success: CheckCircle2, warn: AlertTriangle, error: AlertCircle,
};
function levelColor(level: string): string {
  return ({
    debug: "var(--text-muted)",
    info: "var(--color-blue-light)",
    success: "var(--color-success-light)",
    warn: "#eab308",
    error: "var(--color-danger)",
  } as Record<string, string>)[level] || "var(--text-muted)";
}

onMounted(() => {
  // 立即异步加载，不 await，让 skeleton 先渲染出来
  loadServices(true);
  checkStartupErrors();
  loadRecentLogs();
  // 轮询从 3s 放宽到 5s（后端已做 status 缓存 + 快速返回 + 后台并行刷新）
  svcTimer = window.setInterval(() => loadServices(), 5000);
  logTimer = window.setInterval(loadRecentLogs, 2000);
});

onUnmounted(() => {
  if (svcTimer) clearInterval(svcTimer);
  if (logTimer) clearInterval(logTimer);
});
</script>

<template>
  <div class="max-w-[1100px]">
    <!-- Header -->
    <div class="flex items-center justify-between mb-3">
      <h1 class="text-base font-semibold">仪表板</h1>
      <div class="flex gap-2">
        <button class="btn btn-success btn-sm" :disabled="batchBusy" @click="startAll">{{ batchBusy ? "启动中..." : "全部启动" }}</button>
        <button class="btn btn-danger btn-sm" :disabled="batchBusy" @click="stopAll">{{ batchBusy ? "停止中..." : "全部停止" }}</button>
      </div>
    </div>

    <!-- Skeleton：首次加载前占位，避免空白页闪烁 -->
    <div v-if="!loaded" class="grid grid-cols-4 gap-3 mb-4">
      <div v-for="i in 4" :key="i" class="svc-card skeleton-card">
        <div class="flex items-start justify-between mb-3">
          <div class="min-w-0 flex-1">
            <div class="skel-bar h-3 w-14 mb-1.5"></div>
            <div class="skel-bar h-2 w-10"></div>
          </div>
          <span class="w-2 h-2 rounded-full shrink-0 mt-1.5" style="background: var(--color-gray-light); opacity: 0.4"></span>
        </div>
        <div class="skel-bar h-3 w-16 mb-3"></div>
        <div class="skel-bar h-7 w-full rounded"></div>
      </div>
    </div>

    <!-- Empty State -->
    <div v-else-if="services.length === 0" class="card py-8 px-6">
      <div class="text-center mb-5">
        <div class="inline-flex w-12 h-12 rounded-full items-center justify-center mb-3"
             style="background: var(--bg-tertiary); color: var(--text-muted)">
          <Store :size="22" />
        </div>
        <p class="text-base font-semibold mb-1">没有发现已安装的服务</p>
        <p class="text-[13px]" style="color: var(--text-muted)">选择一个方式继续：</p>
      </div>
      <div class="grid grid-cols-2 gap-3 max-w-[560px] mx-auto">
        <button class="cta-card" @click="router.push('/store')">
          <Store :size="18" class="mb-2" style="color: #a855f7" />
          <span class="font-semibold text-[13px] mb-0.5">打开软件商店</span>
          <span class="text-[11px]" style="color: var(--text-muted)">一键安装 Nginx / MySQL / PHP / Redis</span>
        </button>
        <button class="cta-card" @click="router.push('/settings')">
          <Settings2 :size="18" class="mb-2" style="color: var(--color-blue-light)" />
          <span class="font-semibold text-[13px] mb-0.5">指定 PHPStudy 路径</span>
          <span class="text-[11px]" style="color: var(--text-muted)">已安装过 PHPStudy？设置里填路径</span>
        </button>
      </div>
    </div>

    <template v-else>
      <!-- Overview Bar -->
      <div class="flex items-center gap-6 mb-4 px-1 text-[13px]" style="color: var(--text-secondary)">
        <div>运行中 <span class="font-semibold" style="color: var(--color-success-light)">{{ runningCount }}</span><span class="opacity-60">/{{ totalCount }}</span></div>
        <div>已停止 <span class="font-semibold" style="color: var(--text-primary)">{{ stoppedCount }}</span></div>
      </div>

      <!-- Main service cards (4 columns) -->
      <div class="grid grid-cols-4 gap-3 mb-4">
        <div v-for="s in mainServices" :key="s.id"
             class="svc-card"
             :class="{ 'is-running': isRunning(s) }">
          <!-- Name & Status -->
          <div class="flex items-start justify-between mb-3">
            <div class="min-w-0">
              <div class="flex items-center gap-1.5">
                <div class="text-sm font-semibold">{{ kindLabel(s.kind) }}</div>
                <span v-if="originBadge(s)" class="text-[9px] px-1.5 py-px rounded font-semibold leading-none"
                      :style="{ background: `${originBadge(s)!.color}22`, color: originBadge(s)!.color }">{{ originBadge(s)!.text }}</span>
              </div>
              <div class="text-[11px] font-mono mt-0.5" style="color: var(--text-muted)">{{ s.version }}</div>
            </div>
            <span class="w-2 h-2 rounded-full shrink-0 mt-1.5 transition-all duration-300"
                  :style="isRunning(s)
                    ? { background: 'var(--color-success-light)', boxShadow: '0 0 8px var(--color-success-light)' }
                    : { background: 'var(--color-gray-light)' }"></span>
          </div>
          <!-- Status text -->
          <div class="text-[13px] mb-3 transition-colors" :style="{ color: isRunning(s) ? 'var(--color-success-light)' : 'var(--text-muted)' }">
            {{ isBusy(s.id) ? "操作中..." : isRunning(s) ? "运行中" : "已停止" }}
          </div>
          <!-- Actions -->
          <div class="flex gap-1.5">
            <template v-if="isBusy(s.id)"><button class="btn btn-secondary btn-sm flex-1" disabled>...</button></template>
            <template v-else-if="!isRunning(s)">
              <button class="btn btn-success btn-sm flex-1" @click="startService(s.id)">启动</button>
            </template>
            <template v-else>
              <button class="btn btn-danger btn-sm flex-1" @click="stopService(s.id)">停止</button>
              <button class="btn btn-secondary btn-sm" @click="restartService(s.id)" title="重启">⟳</button>
            </template>
          </div>
        </div>
      </div>

      <!-- PHP engine combined card -->
      <div v-if="phpServices.length" class="card mb-4">
        <div class="flex items-center justify-between mb-3">
          <div class="text-sm font-semibold">PHP 引擎</div>
          <span class="text-[12px]" style="color: var(--text-muted)">
            <span style="color: var(--color-success-light)" class="font-semibold">{{ phpRunning }}</span>/{{ phpServices.length }} 运行中
          </span>
        </div>
        <div class="flex flex-wrap items-center gap-x-5 gap-y-2 mb-3">
          <div v-for="p in phpServices" :key="p.id" class="flex items-center gap-1.5">
            <span class="w-1.5 h-1.5 rounded-full shrink-0"
                  :style="isRunning(p)
                    ? { background: 'var(--color-success-light)', boxShadow: '0 0 4px var(--color-success-light)' }
                    : { background: 'var(--color-gray-light)' }"></span>
            <span class="text-[13px] font-mono">{{ p.version }}{{ p.variant ? ' '+p.variant : '' }}</span>
          </div>
        </div>
        <div class="flex gap-2">
          <button class="btn btn-success btn-sm" @click="startAllPhp">全部启动</button>
          <button class="btn btn-danger btn-sm" @click="stopAllPhp">全部停止</button>
        </div>
      </div>

      <!-- Recent Activity Log (compact) -->
      <div class="rounded-lg px-3 py-2" style="background: var(--bg-secondary); box-shadow: var(--shadow-card)">
        <div class="flex items-center justify-between mb-1.5">
          <div class="text-[11px] font-semibold uppercase tracking-wider" style="color: var(--text-muted)">活动日志</div>
          <button class="text-[11px] flex items-center gap-0.5 hover:opacity-80 transition-opacity cursor-pointer"
                  style="color: var(--color-blue-light); background: transparent; border: none"
                  @click="logDrawerOpen = true">
            查看全部 <ChevronRight :size="11" />
          </button>
        </div>
        <div v-if="recentLogs.length === 0" class="text-[12px] py-2 text-center" style="color: var(--text-muted)">暂无活动</div>
        <div v-else class="flex flex-col">
          <div v-for="log in recentLogs.slice(0, 5)" :key="log.id"
               class="flex items-center gap-2 px-1 py-0.5 rounded text-[12px]">
            <component :is="levelIconMap[log.level]" :size="11" :style="{ color: levelColor(log.level) }" class="shrink-0" />
            <span class="font-mono text-[10px] shrink-0" style="color: var(--text-muted)">{{ log.timestamp.slice(11, 19) }}</span>
            <span class="truncate" style="color: var(--text-secondary)">{{ log.message }}</span>
          </div>
        </div>
      </div>
    </template>

    <!-- Log Drawer -->
    <LogDrawer :open="logDrawerOpen" @close="logDrawerOpen = false" />
  </div>
</template>

<style scoped>
.svc-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 16px;
  box-shadow: var(--shadow-card);
  transition: box-shadow 200ms ease, border-color 200ms ease;
}
.svc-card:hover {
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.22);
  border-color: var(--border-color-hover, var(--border-color));
}
.svc-card.is-running {
  border-color: rgba(63, 185, 80, 0.25);
}

.cta-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 18px 14px;
  border-radius: 10px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  cursor: pointer;
  transition: transform 180ms ease, box-shadow 180ms ease, border-color 180ms ease;
  color: var(--text-primary);
}
.cta-card:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.22);
  border-color: var(--color-blue);
}
.cta-card:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

/* Skeleton 占位：首次加载前显示，shimmer 动画提示"正在准备" */
.skeleton-card {
  pointer-events: none;
  opacity: 0.75;
}
.skel-bar {
  display: block;
  border-radius: 4px;
  background: linear-gradient(
    90deg,
    var(--bg-tertiary) 0%,
    var(--bg-hover) 50%,
    var(--bg-tertiary) 100%
  );
  background-size: 200% 100%;
  animation: skel-shimmer 1.4s ease-in-out infinite;
}
@keyframes skel-shimmer {
  0% { background-position: 200% 0; }
  100% { background-position: -200% 0; }
}
</style>
