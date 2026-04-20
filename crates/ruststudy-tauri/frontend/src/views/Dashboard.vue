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
// 每类服务当前选中的版本 id（下拉切版本用）。key 是 kind，value 是 ServiceInfo.id
const selectedByKind = ref<Record<string, string>>({});

// 全局 CLI PHP
interface GlobalPhpInfo { version: string | null; bin_dir: string; path_registered: boolean; conflicts: string[]; }
const globalPhp = ref<GlobalPhpInfo>({ version: null, bin_dir: "", path_registered: false, conflicts: [] });
const showConflictHelp = ref(false);
const conflictFixBusy = ref(false);
const globalPhpPick = ref<string>(""); // 下拉当前选的版本
const globalPhpBusy = ref(false);

import { toast } from "../composables/useToast";
import { confirm } from "@tauri-apps/plugin-dialog";
const busyIds = ref<Set<string>>(new Set());
const recentLogs = ref<LogEntry[]>([]);
const logDrawerOpen = ref(false);
const batchBusy = ref(false);

let svcTimer: number | null = null;
let logTimer: number | null = null;
let pauseUntil = 0;

// ==================== Computed ====================

interface KindGroup {
  kind: string;
  all: ServiceInfo[];              // 同类所有已装版本（含 PhpStudy + Store）
  active: ServiceInfo;             // 下拉里当前选中的
  running: ServiceInfo | null;     // 当前同 kind 运行中的那一个（如有）
}

/**
 * 按 kind 分组，每组一张卡。active 的优先级：
 *   1. 用户下拉选的（selectedByKind）
 *   2. 运行中的实例
 *   3. 第一个扫到的
 */
const mainKindGroups = computed<KindGroup[]>(() => {
  const kinds = ["nginx", "apache", "mysql", "redis"];
  const groups: KindGroup[] = [];
  for (const k of kinds) {
    const all = services.value.filter(s => s.kind === k);
    if (all.length === 0) continue;
    const running = all.find(isRunning) ?? null;
    const userPick = all.find(s => s.id === selectedByKind.value[k]);
    const active = userPick ?? running ?? all[0];
    groups.push({ kind: k, all, active, running });
  }
  return groups;
});

function originShort(s: ServiceInfo): string {
  if (s.origin === "phpstudy") return "PS";
  if (s.origin === "manual") return "独立";
  return "商店";
}

/** 在不同版本运行时，从下拉选了另一个版本 + 点"切换" → 停旧启新 */
async function switchToActive(kind: string, g: KindGroup) {
  if (!g.running || g.running.id === g.active.id) return;
  const targetId = g.active.id;
  const targetLabel = `${kindLabel(kind)} v${g.active.version}`;
  busyIds.value.add(targetId);
  pauseAutoRefresh();
  toast.info(`切换到 ${targetLabel}...`);
  try {
    // start_service 后端的 start_with_deps 会先停同 kind 其他版本
    services.value = await invoke("start_service", { id: targetId });
    toast.success(`已切换到 ${targetLabel}`);
  } catch (e) {
    showError(`切换失败: ${e}`);
    // 回退：清用户选择，让 computed 自然拉回真实运行版本
    delete selectedByKind.value[kind];
  } finally {
    busyIds.value.delete(targetId);
  }
}

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

async function loadGlobalPhp() {
  try {
    globalPhp.value = await invoke<GlobalPhpInfo>("get_global_php_version");
    // 下拉默认回落到当前全局版本；没有就取第一个可用 PHP
    if (!globalPhpPick.value) {
      globalPhpPick.value = globalPhp.value.version ?? phpServices.value[0]?.version ?? "";
    }
  } catch (e) { /* 非致命，静默 */ }
}

async function fixConflicts() {
  if (conflictFixBusy.value || !globalPhp.value.conflicts.length) return;
  const list = globalPhp.value.conflicts;
  const ok = await confirm(
    `即将从系统 PATH 清除 ${list.length} 条 PHP 路径：\n\n${list.join('\n')}\n\n需要管理员权限（会弹 UAC 确认）。`,
    { title: "一键修复 PATH 冲突", kind: "warning" }
  );
  if (!ok) return;
  conflictFixBusy.value = true;
  try {
    globalPhp.value = await invoke<GlobalPhpInfo>("fix_global_php_conflicts", { paths: list });
    if (globalPhp.value.conflicts.length === 0) {
      toast.success("清理完成。请**新开** cmd 窗口验证。");
    } else {
      toast.warn(`部分路径仍在系统 PATH 中（${globalPhp.value.conflicts.length} 条）`);
    }
  } catch (e) {
    showError(`修复失败: ${e}`);
  } finally {
    conflictFixBusy.value = false;
  }
}

async function openEnvEditor() {
  try {
    await invoke("open_system_env_editor");
  } catch (e) {
    showError(`打开失败: ${e}`);
  }
}

async function applyGlobalPhp() {
  if (!globalPhpPick.value || globalPhpBusy.value) return;
  const version = globalPhpPick.value;
  const firstTime = !globalPhp.value.version;
  const hint = firstTime
    ? `将 CLI 的 php 命令切到 v${version}。首次使用会把 ~/.ruststudy/bin 加入用户 PATH。继续？`
    : `切换 CLI php 到 v${version}。新开命令行窗口即可生效。继续？`;
  const ok = await confirm(hint, { title: "设置全局 PHP", kind: "info" });
  if (!ok) return;

  globalPhpBusy.value = true;
  try {
    globalPhp.value = await invoke<GlobalPhpInfo>("set_global_php_version", { version });
    const tip = firstTime && globalPhp.value.path_registered
      ? `全局 PHP 已切到 v${version}。请**新开** cmd 窗口跑 \`php -v\` 验证。`
      : `全局 PHP 已切到 v${version}`;
    toast.success(tip);
  } catch (e) {
    showError(`设置失败: ${e}`);
  } finally {
    globalPhpBusy.value = false;
  }
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
  loadGlobalPhp();
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
        <div v-for="g in mainKindGroups" :key="g.kind"
             class="svc-card"
             :class="{ 'is-running': !!g.running }">
          <!-- Name & Status -->
          <div class="flex items-start justify-between mb-3">
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-1.5">
                <div class="text-sm font-semibold">{{ kindLabel(g.kind) }}</div>
                <span v-if="originBadge(g.active)" class="text-[9px] px-1.5 py-px rounded font-semibold leading-none"
                      :style="{ background: `${originBadge(g.active)!.color}22`, color: originBadge(g.active)!.color }">{{ originBadge(g.active)!.text }}</span>
              </div>
              <!-- 单版本：静态显示；多版本：下拉（纯选择，不触发 IPC） -->
              <template v-if="g.all.length === 1">
                <div class="text-[11px] font-mono mt-0.5" style="color: var(--text-muted)">{{ g.active.version }}</div>
              </template>
              <template v-else>
                <select class="version-sel mt-1"
                        :value="g.active.id"
                        @change="selectedByKind[g.kind] = ($event.target as HTMLSelectElement).value">
                  <option v-for="s in g.all" :key="s.id" :value="s.id">
                    {{ g.running?.id === s.id ? '● ' : '' }}v{{ s.version }} [{{ originShort(s) }}]
                  </option>
                </select>
              </template>
            </div>
            <span class="w-2 h-2 rounded-full shrink-0 mt-1.5 transition-all duration-300"
                  :style="g.running
                    ? { background: 'var(--color-success-light)', boxShadow: '0 0 8px var(--color-success-light)' }
                    : { background: 'var(--color-gray-light)' }"></span>
          </div>

          <!-- Status text：选中==运行 时只写"运行中"；不一致时明确提示实际运行版本 -->
          <div class="text-[13px] mb-3 transition-colors"
               :style="{ color: g.running && g.running.id === g.active.id ? 'var(--color-success-light)' : 'var(--text-muted)' }">
            <template v-if="isBusy(g.active.id)">操作中...</template>
            <template v-else-if="!g.running">已停止</template>
            <template v-else-if="g.running.id === g.active.id">运行中</template>
            <template v-else>运行中：v{{ g.running.version }}</template>
          </div>

          <!-- Actions -->
          <div class="flex gap-1.5">
            <template v-if="isBusy(g.active.id)">
              <button class="btn btn-secondary btn-sm flex-1" disabled>...</button>
            </template>
            <template v-else-if="!g.running">
              <!-- 没运行 → 启动选中版本 -->
              <button class="btn btn-success btn-sm flex-1" @click="startService(g.active.id)">启动</button>
            </template>
            <template v-else-if="g.running.id !== g.active.id">
              <!-- 有运行，但不是选中的 → 显式切换按钮 -->
              <button class="btn btn-primary btn-sm flex-1" @click="switchToActive(g.kind, g)">切换到 v{{ g.active.version }}</button>
            </template>
            <template v-else>
              <!-- 选中 = 运行 → 停止/重启 -->
              <button class="btn btn-danger btn-sm flex-1" @click="stopService(g.active.id)">停止</button>
              <button class="btn btn-secondary btn-sm" @click="restartService(g.active.id)" title="重启">⟳</button>
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

        <!-- 全局 CLI PHP：用户命令行 `php -v` 对应的版本 -->
        <div class="flex items-center gap-2 mb-3 pb-3"
             style="border-bottom: 1px solid var(--border-color)">
          <span class="text-[12px] shrink-0" style="color: var(--text-muted)">全局 CLI:</span>
          <select class="version-sel" v-model="globalPhpPick">
            <option v-for="p in phpServices" :key="p.id" :value="p.version">
              v{{ p.version }}{{ p.variant ? ' '+p.variant : '' }}
            </option>
          </select>
          <span v-if="globalPhp.version" class="text-[11px]" style="color: var(--text-muted)">
            当前: v{{ globalPhp.version }}
          </span>
          <span v-else class="text-[11px]" style="color: var(--color-warn, #f59e0b)">未设置</span>
          <button class="btn btn-primary btn-sm ml-auto"
                  :disabled="!globalPhpPick || globalPhpBusy || globalPhpPick === globalPhp.version"
                  @click="applyGlobalPhp">
            {{ globalPhpBusy ? '切换中...' : (globalPhpPick === globalPhp.version ? '已是全局' : '设为全局') }}
          </button>
        </div>

        <!-- 系统 PATH 冲突警告：系统 PATH 排在用户 PATH 前面，老的 PHP 路径会屏蔽我们的 shim -->
        <div v-if="globalPhp.conflicts.length > 0" class="conflict-banner mb-3">
          <div class="flex items-start gap-2">
            <AlertTriangle :size="16" class="shrink-0 mt-0.5" style="color: #f59e0b" />
            <div class="flex-1 text-[12px]">
              <div class="font-semibold mb-1">全局切换可能不生效</div>
              <div class="mb-1" style="color: var(--text-secondary)">
                系统 PATH 里有 {{ globalPhp.conflicts.length }} 条 PHP 目录排在本应用前面，`php -v` 会优先命中它们。
              </div>
              <div class="font-mono text-[11px] mb-2 pl-2" style="color: var(--text-muted)">
                <div v-for="c in globalPhp.conflicts" :key="c">· {{ c }}</div>
              </div>
              <div class="flex items-center gap-2 flex-wrap">
                <button class="btn btn-primary btn-sm"
                        :disabled="conflictFixBusy"
                        @click="fixConflicts">
                  {{ conflictFixBusy ? '修复中...' : '一键修复（需管理员）' }}
                </button>
                <button class="btn btn-secondary btn-sm" @click="openEnvEditor">
                  打开环境变量窗口
                </button>
                <button class="text-[11px] underline cursor-pointer"
                        style="color: var(--color-blue-light); background: transparent; border: none"
                        @click="showConflictHelp = !showConflictHelp">
                  {{ showConflictHelp ? '隐藏手动步骤' : '手动步骤？' }}
                </button>
              </div>
              <div v-if="showConflictHelp" class="mt-2 text-[11px] leading-relaxed"
                   style="color: var(--text-secondary)">
                <ol class="list-decimal pl-4 space-y-0.5">
                  <li>点上面"打开环境变量窗口"按钮</li>
                  <li>在**系统变量**区（非用户变量那栏）双击 `Path`</li>
                  <li>删掉上述列出的那几条，一路确定</li>
                  <li>**新开**命令行窗口，运行 <code>where php</code> 验证（应该只剩 <code>.ruststudy\bin\php.cmd</code>）</li>
                </ol>
              </div>
            </div>
          </div>
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
/* 多版本下拉：小 pill，明显是个能点的东西 */
.version-sel {
  font-size: 12px;
  color: var(--text-primary);
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 2px 20px 2px 8px;
  outline: none;
  cursor: pointer;
  max-width: 100%;
  -webkit-appearance: none;
  appearance: none;
  background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 10 10'><path d='M2 4l3 3 3-3' stroke='%23888' stroke-width='1.3' fill='none' stroke-linecap='round' stroke-linejoin='round'/></svg>");
  background-repeat: no-repeat;
  background-position: right 6px center;
  transition: border-color 150ms ease, background-color 150ms ease;
}
.version-sel:hover { border-color: var(--text-muted); background: var(--bg-hover); }
.version-sel:focus { border-color: var(--color-blue-light); }
.version-sel option { background: var(--bg-secondary); color: var(--text-primary); font-size: 12px; }

/* 系统 PATH 冲突警告条 */
.conflict-banner {
  background: rgba(245, 158, 11, 0.08);
  border: 1px solid rgba(245, 158, 11, 0.35);
  border-radius: 8px;
  padding: 10px 12px;
}
.conflict-banner code {
  padding: 1px 4px;
  background: var(--bg-tertiary);
  border-radius: 3px;
  font-size: 11px;
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
