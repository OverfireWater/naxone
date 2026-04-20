<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LayoutDashboard, Globe, Settings2, Wrench, Store, ChevronLeft, ChevronRight, Minus, Square, X } from "lucide-vue-next";
import ToastContainer from "./components/ToastContainer.vue";

const router = useRouter();
const route = useRoute();
const collapsed = ref(false);
const appWindow = getCurrentWindow();
const isMaximized = ref(false);

function applyTheme(mode: string) {
  if (mode === "auto") {
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    document.documentElement.setAttribute("data-theme", prefersDark ? "dark" : "light");
  } else {
    document.documentElement.setAttribute("data-theme", mode);
  }
}

onMounted(() => {
  const saved = localStorage.getItem("ruststudy-theme") || "dark";
  applyTheme(saved);
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (localStorage.getItem("ruststudy-theme") === "auto") applyTheme("auto");
  });
  window.addEventListener("theme-change", (e) => applyTheme((e as CustomEvent).detail));
});

const menuItems = [
  { path: "/", label: "仪表板", icon: LayoutDashboard, color: "#6380ff" },
  { path: "/vhosts", label: "虚拟主机", icon: Globe, color: "#22c55e" },
  { path: "/store", label: "软件商店", icon: Store, color: "#a855f7" },
  { path: "/config", label: "服务配置", icon: Wrench, color: "#f59e0b" },
  { path: "/settings", label: "设置", icon: Settings2, color: "#94a3b8" },
];

function navigate(path: string) { router.push(path); }
async function minimize() { await appWindow.minimize(); }
async function toggleMaximize() { await appWindow.toggleMaximize(); isMaximized.value = await appWindow.isMaximized(); }
async function close() { await appWindow.close(); }
</script>

<template>
  <div class="flex flex-col h-screen">
    <!-- Titlebar -->
    <div class="flex items-center justify-between h-9 shrink-0 border-b"
         style="background: var(--bg-titlebar); border-color: var(--border-color)"
         data-tauri-drag-region>
      <div class="flex items-center gap-2.5 pl-4" data-tauri-drag-region>
        <div class="w-5 h-5 bg-gradient-to-br from-indigo-500 to-blue-400 rounded-md flex items-center justify-center">
          <span class="text-[10px] font-bold text-white leading-none">R</span>
        </div>
        <span class="text-xs font-medium" style="color: var(--text-muted)" data-tauri-drag-region>RustStudy</span>
      </div>
      <div class="flex h-full">
        <button class="w-11 h-full border-none bg-transparent cursor-pointer flex items-center justify-center transition-colors duration-200 hover:bg-[var(--bg-hover)]"
                style="color: var(--text-muted)" @click="minimize">
          <Minus :size="14" />
        </button>
        <button class="w-11 h-full border-none bg-transparent cursor-pointer flex items-center justify-center transition-colors duration-200 hover:bg-[var(--bg-hover)]"
                style="color: var(--text-muted)" @click="toggleMaximize">
          <Square :size="12" />
        </button>
        <button class="w-11 h-full border-none bg-transparent cursor-pointer flex items-center justify-center transition-colors duration-200 hover:bg-red-500 hover:text-white"
                style="color: var(--text-muted)" @click="close">
          <X :size="14" />
        </button>
      </div>
    </div>

    <!-- Body -->
    <div class="flex flex-1 overflow-hidden">
      <!-- Sidebar -->
      <aside class="flex flex-col shrink-0 transition-all duration-300 ease-in-out border-r"
             :class="collapsed ? 'w-[60px]' : 'w-[220px]'"
             style="background: var(--bg-secondary); border-color: var(--border-color)">
        <nav class="flex-1 p-3 flex flex-col gap-1 mt-1">
          <div
            v-for="item in menuItems"
            :key="item.path"
            class="flex items-center gap-2.5 px-2.5 py-2 rounded-lg cursor-pointer transition-all duration-200"
            :class="[
              route.path === item.path
                ? 'font-semibold'
                : 'hover:bg-[var(--bg-hover)]'
            ]"
            :style="{
              background: route.path === item.path ? 'var(--bg-active)' : undefined,
              color: route.path === item.path ? 'var(--text-primary)' : 'var(--text-secondary)',
            }"
            @click="navigate(item.path)"
          >
            <div class="w-7 h-7 rounded-md flex items-center justify-center shrink-0 transition-all duration-200"
                 :style="{
                   background: route.path === item.path ? item.color : 'var(--bg-tertiary)',
                   color: route.path === item.path ? '#fff' : item.color,
                   boxShadow: route.path === item.path ? `0 2px 8px ${item.color}40` : 'none',
                 }">
              <component :is="item.icon" :size="16" :stroke-width="2" />
            </div>
            <span v-if="!collapsed" class="text-[13px] whitespace-nowrap">{{ item.label }}</span>
          </div>
        </nav>
        <div class="p-3 border-t flex items-center cursor-pointer transition-colors duration-200"
             style="border-color: var(--border-color); color: var(--text-muted)"
             :class="collapsed ? 'justify-center' : ''"
             @click="collapsed = !collapsed">
          <component :is="collapsed ? ChevronRight : ChevronLeft" :size="14" />
          <span v-if="!collapsed" class="text-[11px] ml-auto" style="color: var(--text-muted)">v0.1.0</span>
        </div>
      </aside>

      <!-- Main -->
      <main class="flex-1 overflow-y-auto px-5 py-4" style="background: var(--bg-primary)">
        <router-view />
      </main>
    </div>

    <!-- Global floating toasts -->
    <ToastContainer />
  </div>
</template>
