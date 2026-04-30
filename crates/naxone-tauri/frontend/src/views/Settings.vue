<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "../composables/useToast";
import SelectMenu from "../components/SelectMenu.vue";

interface ConfigDto {
  phpstudy_path: string; www_root: string; active_web_server: string;
  auto_start: string[]; mysql_port: number; redis_port: number;
  log_dir: string; log_retention_days: number;
  stop_services_on_exit: boolean;
}

const config = ref<ConfigDto>({ phpstudy_path: "", www_root: "", active_web_server: "nginx", auto_start: [], mysql_port: 3306, redis_port: 6379, log_dir: "", log_retention_days: 7, stop_services_on_exit: false });
const busy = ref(false);
const saved = ref(false);
const logRetentionOptions = [
  { label: "3 天", value: 3 },
  { label: "7 天", value: 7 },
  { label: "30 天", value: 30 },
  { label: "90 天", value: 90 },
];

// Theme
const themeMode = ref(localStorage.getItem("naxone-theme") || "dark");
function setTheme(mode: string) {
  themeMode.value = mode;
  localStorage.setItem("naxone-theme", mode);
  window.dispatchEvent(new CustomEvent("theme-change", { detail: mode }));
}

function showError(msg: string) { toast.error(String(msg)); }

async function loadConfig() { try { config.value = await invoke("get_config"); } catch (e) { showError("加载配置失败: " + e); } }

async function saveSettings() {
  busy.value = true;
  try {
    const ps = (config.value.phpstudy_path || "").replace(/\\/g, "/").replace(/\/+$/, "");
    if (ps && !config.value.www_root) {
      config.value.www_root = ps + "/WWW";
    }
    await invoke("save_config", { dto: config.value });
    await invoke("rescan_services");
    saved.value = true; setTimeout(() => (saved.value = false), 2000);
  } catch (e) { showError("保存失败: " + e); } finally { busy.value = false; }
}

async function openLogDir() {
  try { await invoke("open_log_dir"); } catch (e) { showError("打开失败: " + e); }
}

async function rescan() {
  busy.value = true;
  try { await invoke("rescan_services"); saved.value = true; setTimeout(() => (saved.value = false), 2000); }
  catch (e) { showError("扫描失败: " + e); } finally { busy.value = false; }
}

function toggleAutoStart(service: string) {
  const idx = config.value.auto_start.indexOf(service);
  if (idx >= 0) config.value.auto_start.splice(idx, 1);
  else config.value.auto_start.push(service);
}

onMounted(() => { loadConfig(); });
</script>

<template>
  <div class="max-w-[640px] has-save-bar">

    <!-- Theme -->
    <div class="card mb-3">
      <h2 class="text-sm font-medium text-content-secondary mb-3">外观主题</h2>
      <div class="flex gap-3">
        <label
          v-for="t in [{v:'dark',icon:'☾',l:'暗色'},{v:'light',icon:'☀',l:'亮色'},{v:'auto',icon:'◐',l:'跟随系统'}]"
          :key="t.v"
          class="flex-1 flex items-center gap-2.5 px-4 py-3 border border-border rounded-lg cursor-pointer transition-all"
          :class="themeMode === t.v ? 'border-accent-blue bg-[rgba(29,78,216,0.1)]' : ''"
          @click="setTheme(t.v)"
        >
          <input type="radio" v-model="themeMode" :value="t.v" class="hidden" />
          <span class="text-lg">{{ t.icon }}</span>
          <span class="text-sm">{{ t.l }}</span>
        </label>
      </div>
    </div>

    <!-- General -->
    <div class="card mb-3">
      <h2 class="text-sm font-medium text-content-secondary mb-3">基本设置</h2>
      <div class="grid grid-cols-1 gap-4">
        <div class="flex flex-col gap-1.5">
          <label class="text-[13px] text-content-secondary font-medium">PHPStudy 安装路径</label>
          <input class="input" v-model="config.phpstudy_path" placeholder="D:\phpstudy_pro" />
          <p class="text-xs text-content-muted mt-1">扫描此目录下的 Extensions 发现已安装的服务；默认站点目录使用 NaxOne 自己管理的 www 路径，不跟随 PHPStudy 的 WWW</p>
        </div>
      </div>
    </div>

    <!-- Web Server -->
    <div class="card mb-3">
      <h2 class="text-sm font-medium text-content-secondary mb-3">Web 服务器</h2>
      <div class="flex gap-3">
        <label
          v-for="ws in [{v:'nginx',c:'#009639',i:'N'},{v:'apache',c:'#d22128',i:'A'}]"
          :key="ws.v"
          class="flex-1 flex items-center gap-2.5 px-4 py-3 border border-border rounded-lg cursor-pointer transition-all"
          :class="config.active_web_server === ws.v ? 'border-accent-blue bg-[rgba(29,78,216,0.1)]' : ''"
        >
          <input type="radio" v-model="config.active_web_server" :value="ws.v" class="hidden" />
          <span class="w-6 h-6 rounded flex items-center justify-center text-xs font-bold text-white" :style="{ background: ws.c }">{{ ws.i }}</span>
          <span class="text-sm capitalize">{{ ws.v }}</span>
        </label>
      </div>
      <p class="text-xs text-content-muted mt-2.5">"全部启动" 时只启动选中的 Web 服务器</p>
    </div>

    <!-- Ports -->
    <div class="card mb-3">
      <h2 class="text-sm font-medium text-content-secondary mb-3">端口配置</h2>
      <div class="grid grid-cols-2 gap-4">
        <div class="flex flex-col gap-1.5">
          <label class="text-[13px] text-content-secondary font-medium">MySQL 端口</label>
          <input class="input" type="number" v-model.number="config.mysql_port" min="1" max="65535" />
        </div>
        <div class="flex flex-col gap-1.5">
          <label class="text-[13px] text-content-secondary font-medium">Redis 端口</label>
          <input class="input" type="number" v-model.number="config.redis_port" min="1" max="65535" />
        </div>
      </div>
    </div>

    <!-- Auto Start -->
    <div class="card mb-3">
      <h2 class="text-sm font-medium text-content-secondary mb-3">自动启动</h2>
      <p class="text-xs text-content-muted mb-3">选择应用启动时自动启动的服务</p>
      <div class="flex flex-wrap gap-3">
        <label v-for="svc in ['nginx','apache','mysql','redis']" :key="svc" class="flex items-center gap-2 text-sm cursor-pointer">
          <input type="checkbox" :checked="config.auto_start.includes(svc)" @change="toggleAutoStart(svc)" class="accent-accent-success w-4 h-4" />
          <span class="capitalize">{{ svc }}</span>
        </label>
      </div>
    </div>

    <!-- Exit behavior -->
    <div class="card mb-3">
      <h2 class="text-sm font-medium text-content-secondary mb-3">退出行为</h2>
      <label class="flex items-center gap-2 text-sm cursor-pointer mb-2">
        <input type="checkbox" v-model="config.stop_services_on_exit" class="accent-accent-success w-4 h-4" />
        <span>退出应用时自动停止所有服务</span>
      </label>
      <p class="text-xs text-content-muted">仅托盘菜单“退出”生效；点窗口右上角关闭只是最小化到托盘。</p>
    </div>

    <!-- Log Settings -->
    <div class="card mb-3">
      <h2 class="text-sm font-medium text-content-secondary mb-3">日志设置</h2>
      <div class="grid grid-cols-2 gap-4">
        <div class="flex flex-col gap-1.5 col-span-2">
          <label class="text-[13px] text-content-secondary font-medium">日志目录 <span class="text-xs text-content-muted font-normal">留空使用默认（exe 同级 logs/）</span></label>
          <div class="flex gap-2">
            <input class="input flex-1" v-model="config.log_dir" placeholder="默认位置" />
            <button class="btn btn-secondary btn-sm" @click="openLogDir">打开目录</button>
          </div>
        </div>
        <div class="flex flex-col gap-1.5">
          <label class="text-[13px] text-content-secondary font-medium">保留天数</label>
          <SelectMenu v-model="config.log_retention_days" :options="logRetentionOptions" full-width trigger-class="input" />
        </div>
      </div>
    </div>

    <div class="save-bar">
      <button class="btn btn-success btn-sm" @click="saveSettings" :disabled="busy">{{ busy ? "保存中..." : "保存设置" }}</button>
      <button class="btn btn-secondary btn-sm" @click="rescan" :disabled="busy">重新扫描服务</button>
      <span v-if="saved" class="saved-msg">已保存</span>
    </div>
  </div>
</template>
