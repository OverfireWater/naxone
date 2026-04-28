<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { CheckCircle2, Download, AlertCircle, Trash2 } from "lucide-vue-next";

interface PackageVersion {
  version: string;
  url: string;
  sha256: string | null;
  size_mb: number | null;
  exe_rel: string;
  variant: string | null;
}

interface PackageEntry {
  name: string;
  display_name: string;
  category: string;
  brand_color: string;
  brand_letter: string;
  homepage?: string;
  description?: string;
  versions: PackageVersion[];
}

const props = defineProps<{
  pkg: PackageEntry;
  installedVersions: string[];
}>();

type Phase = "idle" | "starting" | "downloading" | "extracting" | "done" | "failed";

const selectedVersion = ref<string>(props.pkg.versions[0]?.version || "");
const phase = ref<Phase>("idle");
const progress = ref(0);
const downloadedMB = ref<number | null>(null);
const errorMsg = ref("");
const confirmUninstall = ref(false);
const uninstalling = ref(false);

let unlisten: UnlistenFn | null = null;

const isSelectedInstalled = computed(() => props.installedVersions.includes(selectedVersion.value));
const currentVersion = computed(() => props.pkg.versions.find(v => v.version === selectedVersion.value));
const isBusy = computed(() => phase.value === "starting" || phase.value === "downloading" || phase.value === "extracting");

async function doInstall() {
  if (isBusy.value || isSelectedInstalled.value) return;

  const singleOnly = props.pkg.name === "nginx" || props.pkg.name === "apache";
  if (singleOnly) {
    const other = props.installedVersions.find(v => v !== selectedVersion.value);
    if (other) {
      const ok = await confirm(
        `${props.pkg.display_name} 限装一个版本，当前已安装 v${other}，安装 v${selectedVersion.value} 会先卸载它。确定吗？`,
        { title: "替换已有版本", kind: "warning" }
      );
      if (!ok) return;
      try {
        await invoke("uninstall_package", { name: props.pkg.name, version: other });
      } catch (e) {
        phase.value = "failed";
        errorMsg.value = `无法替换已有版本：${e}`;
        return;
      }
    }
  }

  phase.value = "starting";
  progress.value = 0;
  downloadedMB.value = null;
  errorMsg.value = "";
  try {
    await invoke("install_package", { name: props.pkg.name, version: selectedVersion.value });
  } catch (e) {
    phase.value = "failed";
    errorMsg.value = String(e);
  }
}

function handleEvent(ev: any) {
  const p = ev.payload;
  if (p.name !== props.pkg.name || p.version !== selectedVersion.value) return;
  switch (p.phase) {
    case "started":
      phase.value = "downloading";
      progress.value = 0;
      downloadedMB.value = null;
      break;
    case "progress":
      phase.value = "downloading";
      progress.value = Math.round(p.pct || 0);
      if (typeof p.downloaded === "number") downloadedMB.value = p.downloaded / 1024 / 1024;
      break;
    case "extracting":
      phase.value = "extracting";
      progress.value = 100;
      break;
    case "done":
      phase.value = "done";
      progress.value = 100;
      emit("installed", { name: props.pkg.name, version: p.version });
      break;
    case "failed":
      phase.value = "failed";
      errorMsg.value = p.reason || "安装失败";
      break;
  }
}

const emit = defineEmits<{
  installed: [{ name: string; version: string }];
  uninstalled: [{ name: string; version: string }];
}>();

async function doUninstall() {
  if (uninstalling.value || !isSelectedInstalled.value) return;
  uninstalling.value = true;
  errorMsg.value = "";
  try {
    await invoke("uninstall_package", { name: props.pkg.name, version: selectedVersion.value });
    confirmUninstall.value = false;
    phase.value = "idle";
    progress.value = 0;
    downloadedMB.value = null;
    emit("uninstalled", { name: props.pkg.name, version: selectedVersion.value });
  } catch (e) {
    phase.value = "failed";
    errorMsg.value = String(e);
    confirmUninstall.value = false;
  } finally {
    uninstalling.value = false;
  }
}

onMounted(async () => {
  unlisten = await listen("install-progress", handleEvent);
});
onUnmounted(() => {
  if (unlisten) unlisten();
});

watch(selectedVersion, () => {
  if (phase.value === "done" || phase.value === "failed") {
    phase.value = "idle";
    progress.value = 0;
    downloadedMB.value = null;
    errorMsg.value = "";
  }
});
</script>

<template>
  <div class="store-card">
    <div class="flex items-start gap-3 mb-3">
      <div class="w-10 h-10 rounded-lg flex items-center justify-center shrink-0 text-white font-bold text-lg"
           :style="{ background: pkg.brand_color, boxShadow: `0 2px 8px ${pkg.brand_color}55` }">
        {{ pkg.brand_letter }}
      </div>
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-1.5">
          <div class="text-sm font-semibold truncate">{{ pkg.display_name }}</div>
          <span v-if="installedVersions.length > 0" class="text-[10px] px-1.5 py-px rounded font-semibold shrink-0"
                style="background: rgba(34,197,94,0.18); color: var(--color-success-light)">
            {{ installedVersions.length }} 个版本已装
          </span>
        </div>
        <div v-if="pkg.description" class="text-[11px] mt-0.5 line-clamp-2" style="color: var(--text-muted)">
          {{ pkg.description }}
        </div>
      </div>
    </div>

    <div class="flex items-center gap-2 mb-3">
      <select class="input sel flex-1" v-model="selectedVersion" :disabled="isBusy">
        <option v-for="v in pkg.versions" :key="v.version" :value="v.version">
          v{{ v.version }}<template v-if="v.variant"> · {{ v.variant }}</template>
          <template v-if="installedVersions.includes(v.version)"> · 已安装</template>
        </option>
      </select>
      <span v-if="currentVersion?.size_mb" class="text-[11px] font-mono shrink-0" style="color: var(--text-muted)">~{{ currentVersion.size_mb }}MB</span>
    </div>

    <div class="store-action">
      <div v-if="isSelectedInstalled && phase !== 'downloading' && phase !== 'extracting' && phase !== 'starting'" class="installed-wrap">
        <template v-if="confirmUninstall">
          <button class="btn btn-danger btn-sm flex-1 flex items-center justify-center gap-1" :disabled="uninstalling" @click="doUninstall">
            {{ uninstalling ? "卸载中..." : "确认卸载" }}
          </button>
          <button class="btn btn-secondary btn-sm !px-2" :disabled="uninstalling" @click="confirmUninstall = false">×</button>
        </template>
        <template v-else>
          <div class="installed-badge flex-1">
            <CheckCircle2 :size="14" />
            <span>已安装 v{{ selectedVersion }}</span>
          </div>
          <button class="btn btn-secondary btn-sm !px-2 hover:!text-red-400 transition-colors" title="卸载" @click="confirmUninstall = true">
            <Trash2 :size="13" />
          </button>
        </template>
      </div>

      <div v-else-if="isBusy || phase === 'done'" class="progress-wrap">
        <div class="progress-bar">
          <div class="progress-fill" :style="{ width: progress + '%' }"></div>
        </div>
        <span class="progress-text">
          <template v-if="phase === 'starting'">下载中…</template>
          <template v-else-if="phase === 'extracting'">解压中…</template>
          <template v-else-if="phase === 'done'">完成</template>
          <template v-else-if="phase === 'downloading' && progress > 0">{{ progress }}%</template>
          <template v-else-if="phase === 'downloading' && downloadedMB !== null">下载中 {{ downloadedMB.toFixed(1) }} MB</template>
          <template v-else>下载中…</template>
        </span>
      </div>

      <div v-else-if="phase === 'failed'" class="failed-wrap">
        <div class="failed-msg" :title="errorMsg">
          <AlertCircle :size="12" />
          <span class="truncate">{{ errorMsg || "安装失败" }}</span>
        </div>
        <button class="btn btn-secondary btn-sm" @click="doInstall">重试</button>
      </div>

      <button v-else class="btn btn-success btn-sm w-full flex items-center justify-center gap-1.5" @click="doInstall">
        <Download :size="13" />安装
      </button>
    </div>
  </div>
</template>

<style scoped>
.store-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 14px;
  box-shadow: var(--shadow-card);
  display: flex;
  flex-direction: column;
  transition: box-shadow 200ms ease, border-color 200ms ease;
}
.store-card:hover {
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.22);
  border-color: var(--border-color-hover, var(--border-color));
}

.line-clamp-2 {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.store-action {
  margin-top: auto;
  min-height: 32px;
}

.installed-wrap {
  display: flex;
  align-items: stretch;
  gap: 6px;
}
.installed-badge {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 10px;
  border-radius: 6px;
  background: rgba(34, 197, 94, 0.1);
  color: var(--color-success-light);
  font-size: 12px;
  font-weight: 600;
}

.progress-wrap {
  position: relative;
  width: 100%;
  height: 32px;
  border-radius: 6px;
  background: var(--bg-tertiary);
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}
.progress-bar {
  position: absolute;
  inset: 0;
}
.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--color-success) 0%, var(--color-success-light) 100%);
  transition: width 200ms ease-out;
}
.progress-text {
  position: relative;
  z-index: 1;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-primary);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

.failed-wrap {
  display: flex;
  gap: 6px;
  align-items: center;
}
.failed-msg {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--color-danger-light);
  background: rgba(239, 68, 68, 0.12);
  border: 1px solid rgba(239, 68, 68, 0.3);
  border-radius: 6px;
  padding: 6px 8px;
}
</style>
