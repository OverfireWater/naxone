<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { CheckCircle2, Download, AlertCircle, Trash2 } from "lucide-vue-next";
import SelectMenu from "./SelectMenu.vue";

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

interface InstalledInfo {
  name: string;
  version: string;
  install_path: string;
  source: string; // "store" | "system"
}

interface UninstallPreview {
  name: string;
  version: string;
  install_path: string;
  will_delete: string[];
  will_keep: string[];
}

const props = defineProps<{
  pkg: PackageEntry;
  installedVersions: InstalledInfo[];
}>();

type Phase = "idle" | "starting" | "downloading" | "extracting" | "done" | "failed";
type UninstallMode = "none" | "confirm-store" | "confirm-unlink" | "preview-deep";

// system-only 包（源全在 mirror，没装的话本地 versions 列表是空的）选第一个版本（如果有 mirror 数据）
const initialVersion = props.pkg.versions[0]?.version
  || props.installedVersions[0]?.version
  || "";
const selectedVersion = ref<string>(initialVersion);
const phase = ref<Phase>("idle");
const progress = ref(0);
const downloadedMB = ref<number | null>(null);
const errorMsg = ref("");
const uninstallMode = ref<UninstallMode>("none");
const uninstalling = ref(false);
const preview = ref<UninstallPreview | null>(null);

let unlisten: UnlistenFn | null = null;

const installedRecord = computed<InstalledInfo | undefined>(() =>
  props.installedVersions.find(i => i.version === selectedVersion.value)
);
const isSelectedInstalled = computed(() => installedRecord.value !== undefined);
const isSystemInstalled = computed(() => installedRecord.value?.source === "system");
/// 该工具在系统级已装任意版本（不一定是当前选中的版本）。
/// composer / nvm 系统已装时，无论选哪个版本，都不让通过 NaxOne 装新版（避免覆盖用户环境）。
const hasAnySystemInstall = computed(() => props.installedVersions.some(i => i.source === "system"));
const systemInstallRecord = computed(() => props.installedVersions.find(i => i.source === "system"));
const currentVersion = computed(() => props.pkg.versions.find(v => v.version === selectedVersion.value));
const isBusy = computed(() => phase.value === "starting" || phase.value === "downloading" || phase.value === "extracting");
const versionOptions = computed(() => props.pkg.versions.map((v) => {
  const inst = props.installedVersions.find(i => i.version === v.version);
  const tag = inst ? (inst.source === "system" ? " · 系统已装" : " · 已安装") : "";
  return {
    label: `v${v.version}${v.variant ? ` · ${v.variant}` : ""}${tag}`,
    value: v.version,
  };
}));

// 系统已装的版本：如果 packages.versions 没有这条（mirror 拉不到时），SelectMenu 会缺，单独补
const extraSystemOptions = computed(() => {
  const known = new Set(props.pkg.versions.map(v => v.version));
  return props.installedVersions
    .filter(i => i.source === "system" && !known.has(i.version))
    .map(i => ({
      label: `v${i.version} · 系统已装`,
      value: i.version,
    }));
});
const allVersionOptions = computed(() => [...versionOptions.value, ...extraSystemOptions.value]);

async function doInstall() {
  if (isBusy.value || isSelectedInstalled.value) return;

  const singleOnly = props.pkg.name === "nginx" || props.pkg.name === "apache";
  if (singleOnly) {
    const other = props.installedVersions.find(i => i.version !== selectedVersion.value);
    if (other) {
      const ok = await confirm(
        `${props.pkg.display_name} 限装一个版本，当前已安装 v${other.version}，安装 v${selectedVersion.value} 会先卸载它。确定吗？`,
        { title: "替换已有版本", kind: "warning" }
      );
      if (!ok) return;
      try {
        await invoke("uninstall_package", { name: props.pkg.name, version: other.version });
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

/// store 来源：现状逻辑（删 NaxOne 装的目录 + 清环境）
async function doUninstallStore() {
  if (uninstalling.value || !isSelectedInstalled.value) return;
  uninstalling.value = true;
  errorMsg.value = "";
  try {
    await invoke("uninstall_package", { name: props.pkg.name, version: selectedVersion.value });
    uninstallMode.value = "none";
    phase.value = "idle";
    progress.value = 0;
    downloadedMB.value = null;
    emit("uninstalled", { name: props.pkg.name, version: selectedVersion.value });
  } catch (e) {
    phase.value = "failed";
    errorMsg.value = String(e);
    uninstallMode.value = "none";
  } finally {
    uninstalling.value = false;
  }
}

/// system 来源「解除关联」：仅清环境变量
async function doUnlinkSystem() {
  if (uninstalling.value || !isSystemInstalled.value) return;
  uninstalling.value = true;
  errorMsg.value = "";
  try {
    const report: any = await invoke("unlink_system_tool", { name: props.pkg.name });
    uninstallMode.value = "none";
    if (report.errors && report.errors.length > 0) {
      errorMsg.value = `部分失败: ${report.errors.join("; ")}`;
      phase.value = "failed";
    } else {
      phase.value = "idle";
    }
    emit("uninstalled", { name: props.pkg.name, version: selectedVersion.value });
  } catch (e) {
    phase.value = "failed";
    errorMsg.value = String(e);
    uninstallMode.value = "none";
  } finally {
    uninstalling.value = false;
  }
}

/// system 来源「彻底卸载」：先拉 preview 展示路径详情
async function showDeepPreview() {
  if (!isSystemInstalled.value) return;
  errorMsg.value = "";
  try {
    preview.value = await invoke("preview_system_tool_uninstall", { name: props.pkg.name }) as UninstallPreview;
    uninstallMode.value = "preview-deep";
  } catch (e) {
    errorMsg.value = String(e);
    phase.value = "failed";
  }
}

async function doDeepUninstallSystem() {
  if (uninstalling.value || !isSystemInstalled.value) return;
  uninstalling.value = true;
  errorMsg.value = "";
  try {
    const report: any = await invoke("deep_uninstall_system_tool", { name: props.pkg.name });
    uninstallMode.value = "none";
    preview.value = null;
    if (report.errors && report.errors.length > 0) {
      errorMsg.value = `部分失败: ${report.errors.join("; ")}`;
      phase.value = "failed";
    } else {
      phase.value = "idle";
    }
    emit("uninstalled", { name: props.pkg.name, version: selectedVersion.value });
  } catch (e) {
    phase.value = "failed";
    errorMsg.value = String(e);
    uninstallMode.value = "none";
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
  uninstallMode.value = "none";
  preview.value = null;
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
          <div class="text-[16px] font-semibold truncate">{{ pkg.display_name }}</div>
          <span v-if="installedVersions.length > 0" class="text-[13px] px-1.5 py-px rounded font-semibold shrink-0"
                style="background: rgba(34,197,94,0.18); color: var(--color-success-light)">
            {{ installedVersions.length }} 个版本已装
          </span>
        </div>
        <div v-if="pkg.description" class="text-[13px] mt-0.5 line-clamp-2" style="color: var(--text-muted)">
          {{ pkg.description }}
        </div>
      </div>
    </div>

    <div class="flex items-center gap-2 mb-3">
      <SelectMenu v-model="selectedVersion" :options="allVersionOptions" :disabled="isBusy" full-width trigger-class="input" />
      <span v-if="currentVersion?.size_mb" class="text-[13px] font-mono shrink-0" style="color: var(--text-muted)">~{{ currentVersion.size_mb }}MB</span>
    </div>

    <div class="store-action">
      <div v-if="isSelectedInstalled && phase !== 'downloading' && phase !== 'extracting' && phase !== 'starting'" class="installed-stack">
        <div class="installed-wrap">
          <!-- store 来源的简单确认 -->
          <template v-if="uninstallMode === 'confirm-store'">
            <button class="btn btn-danger btn-sm flex-1 flex items-center justify-center gap-1" :disabled="uninstalling" @click="doUninstallStore">
              {{ uninstalling ? "卸载中..." : "确认卸载" }}
            </button>
            <button class="btn btn-secondary btn-sm !px-2" :disabled="uninstalling" @click="uninstallMode = 'none'">×</button>
          </template>

          <!-- system 来源的解除关联确认 -->
          <template v-else-if="uninstallMode === 'confirm-unlink'">
            <button class="btn btn-secondary btn-sm flex-1 flex items-center justify-center gap-1" :disabled="uninstalling" @click="doUnlinkSystem">
              {{ uninstalling ? "处理中..." : "确认解除关联" }}
            </button>
            <button class="btn btn-secondary btn-sm !px-2" :disabled="uninstalling" @click="uninstallMode = 'none'">×</button>
          </template>

          <!-- 默认状态：已装徽章 + 卸载按钮组 -->
          <template v-else>
            <div class="installed-badge flex-1" :class="{ 'badge-system': isSystemInstalled }">
              <CheckCircle2 :size="14" />
              <span>{{ isSystemInstalled ? `系统已装 v${selectedVersion}` : `已安装 v${selectedVersion}` }}</span>
            </div>
            <template v-if="isSystemInstalled">
              <button class="btn btn-secondary btn-sm !px-2 transition-colors" title="解除关联（仅清环境变量）" @click="uninstallMode = 'confirm-unlink'">
                解除
              </button>
              <button class="btn btn-secondary btn-sm !px-2 hover:!text-red-400 transition-colors" title="彻底卸载（删核心文件 + 清环境）" @click="showDeepPreview">
                <Trash2 :size="13" />
              </button>
            </template>
            <template v-else>
              <button class="btn btn-secondary btn-sm !px-2 hover:!text-red-400 transition-colors" title="卸载" @click="uninstallMode = 'confirm-store'">
                <Trash2 :size="13" />
              </button>
            </template>
          </template>
        </div>

        <!-- 卸载/解除失败时的错误条；点 × 关掉 -->
        <div v-if="errorMsg" class="inline-error">
          <AlertCircle :size="11" class="shrink-0" />
          <span class="truncate" :title="errorMsg">{{ errorMsg }}</span>
          <button class="inline-error-close" type="button" @click="errorMsg = ''" title="关闭">×</button>
        </div>
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

      <!-- 系统已装但选了别的版本：不允许通过 NaxOne 装新版本 -->
      <div v-else-if="hasAnySystemInstall" class="failed-wrap">
        <div class="failed-msg" :title="`系统已装 v${systemInstallRecord?.version}，路径: ${systemInstallRecord?.install_path}`" style="color: #fbbf24; background: rgba(245, 158, 11, 0.12); border-color: rgba(245, 158, 11, 0.3);">
          <AlertCircle :size="12" />
          <span class="truncate">系统已装 v{{ systemInstallRecord?.version }}，避免冲突已禁用</span>
        </div>
      </div>

      <button v-else class="btn btn-success btn-sm w-full flex items-center justify-center gap-1.5" @click="doInstall">
        <Download :size="13" />安装
      </button>
    </div>
  </div>

  <!-- 彻底卸载预览模态：列出会删/会留路径，让用户最终确认 -->
  <div v-if="uninstallMode === 'preview-deep' && preview" class="deep-modal-overlay" @click.self="uninstallMode = 'none'">
    <div class="deep-modal">
      <div class="deep-modal-header">
        <span class="font-semibold">彻底卸载 {{ pkg.display_name }} v{{ preview.version }}</span>
        <button class="btn btn-secondary btn-sm !px-2" :disabled="uninstalling" @click="uninstallMode = 'none'">×</button>
      </div>
      <div class="deep-modal-body">
        <div class="text-[13px] mb-2" style="color: var(--text-muted)">
          系统已装路径：<span class="font-mono">{{ preview.install_path }}</span>
        </div>

        <div v-if="preview.will_delete.length > 0" class="section">
          <div class="section-title text-red-400">将删除（{{ preview.will_delete.length }} 项）</div>
          <ul class="path-list">
            <li v-for="p in preview.will_delete" :key="'d-'+p" class="font-mono">{{ p }}</li>
          </ul>
        </div>
        <div class="section">
          <div class="section-title text-yellow-400">将保留（如不需要请手动删除）</div>
          <ul class="path-list">
            <li v-for="p in preview.will_keep" :key="'k-'+p" class="font-mono">{{ p }}</li>
            <li v-if="preview.will_keep.length === 0" style="color: var(--text-muted)">无</li>
          </ul>
        </div>
        <div class="section">
          <div class="section-title">将清除</div>
          <ul class="path-list">
            <li v-if="pkg.name === 'nvm'">用户环境变量 NVM_HOME / NVM_SYMLINK</li>
            <li>用户 PATH 中的相关条目</li>
          </ul>
        </div>
        <div v-if="errorMsg" class="text-[13px] text-red-400 mt-2">{{ errorMsg }}</div>
      </div>
      <div class="deep-modal-actions">
        <button class="btn btn-secondary btn-sm" :disabled="uninstalling" @click="uninstallMode = 'none'">取消</button>
        <button class="btn btn-danger btn-sm" :disabled="uninstalling" @click="doDeepUninstallSystem">
          {{ uninstalling ? "卸载中..." : "彻底卸载" }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.store-card {
  background: var(--bg-secondary);
  backdrop-filter: var(--bg-glass-blur);
  -webkit-backdrop-filter: var(--bg-glass-blur);
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

.installed-stack {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.installed-wrap {
  display: flex;
  align-items: stretch;
  gap: 6px;
}
.inline-error {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  border-radius: 6px;
  background: rgba(239, 68, 68, 0.12);
  color: #fca5a5;
  font-size: 13px;
  line-height: 1.3;
}
.inline-error-close {
  margin-left: auto;
  background: transparent;
  border: none;
  color: inherit;
  cursor: pointer;
  font-size: 14px;
  line-height: 1;
  padding: 0 4px;
}
.inline-error-close:hover { opacity: 0.7; }
.installed-badge {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 10px;
  border-radius: 6px;
  background: rgba(34, 197, 94, 0.1);
  color: var(--color-success-light);
  font-size: 13px;
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
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

.failed-wrap {
  display: flex;
  gap: 6px;
  align-items: center;
}

.badge-system {
  background: rgba(245, 158, 11, 0.12);
  color: #fbbf24;
}

.deep-modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  padding: 24px;
}
.deep-modal {
  background: var(--bg-secondary);
  backdrop-filter: var(--bg-glass-blur);
  -webkit-backdrop-filter: var(--bg-glass-blur);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  width: 100%;
  max-width: 560px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
}
.deep-modal-header {
  padding: 12px 14px;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 13px;
}
.deep-modal-body {
  padding: 12px 14px;
  overflow-y: auto;
  flex: 1;
}
.deep-modal-actions {
  padding: 10px 14px;
  border-top: 1px solid var(--border-color);
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.section {
  margin-bottom: 10px;
}
.section-title {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 4px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.path-list {
  list-style: none;
  padding: 0;
  margin: 0;
  font-size: 13px;
  background: var(--bg-tertiary);
  border-radius: 4px;
  padding: 6px 8px;
  max-height: 140px;
  overflow-y: auto;
}
.path-list li {
  padding: 2px 0;
  color: var(--text-secondary, var(--text-primary));
  word-break: break-all;
}
.failed-msg {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--color-danger-light);
  background: rgba(239, 68, 68, 0.12);
  border: 1px solid rgba(239, 68, 68, 0.3);
  border-radius: 6px;
  padding: 6px 8px;
}
</style>
