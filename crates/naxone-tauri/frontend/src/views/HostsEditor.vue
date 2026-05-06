<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { toast } from "../composables/useToast";
import { onTextareaTab } from "../composables/useTextareaTab";

const content = ref("");
const original = ref("");
const busy = ref(false);
const hostsPath = ref("");

const dirty = computed(() => content.value !== original.value);

function showError(msg: string) {
  toast.error(String(msg));
}

async function loadHosts() {
  busy.value = true;
  try {
    hostsPath.value = await invoke<string>("get_hosts_file_path");
    const text = await invoke<string>("get_hosts_text");
    content.value = text;
    original.value = text;
  } catch (e) {
    showError("加载 hosts 失败: " + e);
  } finally {
    busy.value = false;
  }
}

async function saveHosts() {
  if (busy.value) return;
  busy.value = true;
  try {
    await invoke("save_hosts_text", { text: content.value });
    original.value = content.value;
    toast.success("hosts 保存成功");
  } catch (e) {
    const msg = String(e ?? "");
    if (msg.startsWith("PERMISSION_DENIED:")) {
      const ok = await confirm("保存 hosts 需要管理员权限，是否继续提权保存？", {
        title: "需要管理员权限",
        kind: "warning",
      });
      if (!ok) {
        toast.info("已取消提权保存");
        return;
      }
      try {
        await invoke("save_hosts_text_elevated", { text: content.value });
        original.value = content.value;
        toast.success("提权保存 hosts 成功");
      } catch (e2) {
        showError("提权保存失败: " + e2);
      }
    } else {
      showError("保存失败: " + e);
    }
  } finally {
    busy.value = false;
  }
}

async function openHostsExternally() {
  try {
    const path = hostsPath.value || await invoke<string>("get_hosts_file_path");
    await invoke("open_file", { path });
  } catch (e) {
    showError("打开 hosts 文件失败: " + e);
  }
}

onMounted(loadHosts);
</script>

<template>
  <div class="max-w-[960px]">
    <div class="card mb-3">
      <div class="flex items-center justify-between mb-2">
        <div class="text-[16px] font-medium text-content-secondary">系统 Hosts 文件</div>
        <button class="btn btn-secondary btn-sm" @click="openHostsExternally">系统编辑器打开</button>
      </div>
      <div class="text-[13px] text-content-muted mb-2 break-all">{{ hostsPath || "加载中..." }}</div>
      <textarea
        class="input font-mono text-[13px] leading-5 w-full min-h-[420px]"
        style="resize: vertical"
        v-model="content"
        spellcheck="false"
        @keydown="onTextareaTab"
        placeholder="# 在这里编辑 hosts 内容"
      ></textarea>
    </div>

    <div class="flex items-center gap-3 mt-3 pb-2">
      <button class="btn btn-success btn-sm" :disabled="busy || !dirty" @click="saveHosts">
        {{ busy ? "保存中..." : "保存 Hosts" }}
      </button>
      <button class="btn btn-secondary btn-sm" :disabled="busy" @click="loadHosts">重新加载</button>
      <span v-if="dirty" class="saved-msg" style="color: var(--color-warn, #f59e0b)">有未保存修改</span>
    </div>
  </div>
</template>
