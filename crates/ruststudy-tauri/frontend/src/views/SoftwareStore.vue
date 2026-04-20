<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw } from "lucide-vue-next";
import StoreCard from "../components/StoreCard.vue";
import { toast } from "../composables/useToast";

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

interface InstalledPackage { name: string; version: string; install_path: string; }

const packages = ref<PackageEntry[]>([]);
const installed = ref<InstalledPackage[]>([]);
const activeCat = ref<string>("all");
const refreshing = ref(false);

const categories = [
  { key: "all", label: "全部" },
  { key: "web", label: "Web 服务器" },
  { key: "db", label: "数据库" },
  { key: "php", label: "PHP" },
  { key: "cache", label: "缓存" },
];

async function loadPackages() {
  try { packages.value = await invoke("list_packages"); }
  catch (e) { toast.error("加载清单失败: " + e); }
}

async function refreshIndex() {
  if (refreshing.value) return;
  refreshing.value = true;
  try {
    packages.value = await invoke("refresh_package_index");
  } catch (e) {
    toast.error("刷新失败: " + e);
  } finally {
    refreshing.value = false;
  }
}

async function loadInstalled() {
  try { installed.value = await invoke("get_installed_packages"); }
  catch (e) { toast.error("获取已装列表失败: " + e); }
}

function installedVersionsOf(name: string): string[] {
  return installed.value.filter(i => i.name === name).map(i => i.version);
}

const visiblePackages = computed(() => {
  if (activeCat.value === "all") return packages.value;
  return packages.value.filter(p => p.category === activeCat.value);
});

const totalCount = computed(() => packages.value.length);
const categoryCount = (cat: string) => {
  if (cat === "all") return totalCount.value;
  return packages.value.filter(p => p.category === cat).length;
};

function onInstalled() {
  // An install finished — refresh installed list so card state updates.
  loadInstalled();
}

function onUninstalled() {
  // An uninstall finished — refresh so "已安装" badge goes away.
  loadInstalled();
}

onMounted(() => {
  loadPackages();
  loadInstalled();
});
</script>

<template>
  <div class="max-w-[1100px]">
    <!-- Header -->
    <div class="flex items-center justify-between mb-3">
      <div>
        <h1 class="text-base font-semibold">软件商店</h1>
        <p class="text-[12px] mt-0.5" style="color: var(--text-muted)">选择软件和版本，一键下载安装到本应用</p>
      </div>
      <button class="btn btn-secondary btn-sm flex items-center gap-1.5"
              @click="refreshIndex" :disabled="refreshing"
              :title="refreshing ? '刷新中...' : '强制从官方源拉取最新版本列表（默认缓存 6 小时）'">
        <RefreshCw :size="13" :class="{ 'spin': refreshing }" />
        <span>{{ refreshing ? "刷新中..." : "刷新版本" }}</span>
      </button>
    </div>

    <!-- Category Tabs -->
    <div class="tabs-row">
      <button v-for="c in categories" :key="c.key"
              class="tab" :class="{ active: activeCat === c.key }"
              @click="activeCat = c.key">
        {{ c.label }}
        <span class="count-chip">{{ categoryCount(c.key) }}</span>
      </button>
    </div>

    <!-- Card grid -->
    <div v-if="packages.length === 0" class="card text-center py-10" style="color: var(--text-muted)">
      加载中…
    </div>
    <div v-else class="grid grid-cols-3 gap-3 mt-3">
      <StoreCard
        v-for="pkg in visiblePackages"
        :key="pkg.name"
        :pkg="pkg"
        :installed-versions="installedVersionsOf(pkg.name)"
        @installed="onInstalled"
        @uninstalled="onUninstalled"
      />
    </div>
  </div>
</template>

<style scoped>
.count-chip {
  display: inline-block;
  margin-left: 6px;
  padding: 0 6px;
  border-radius: 4px;
  font-size: 10px;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  background: var(--bg-tertiary);
  color: var(--text-muted);
  line-height: 16px;
}
.tab.active .count-chip {
  background: rgba(99, 128, 255, 0.18);
  color: var(--color-blue-light);
}

@media (max-width: 900px) {
  .grid-cols-3 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}

.spin {
  animation: rs-spin 900ms linear infinite;
}
@keyframes rs-spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
