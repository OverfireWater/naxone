<script setup lang="ts">
import { onMounted, onUnmounted, watch } from "vue";

const props = withDefaults(
  defineProps<{
    open: boolean;
    title: string;
    confirmText?: string;
    cancelText?: string;
    variant?: "default" | "danger";
    busy?: boolean;
  }>(),
  { confirmText: "确认", cancelText: "取消", variant: "default", busy: false },
);

const emit = defineEmits<{ (e: "confirm"): void; (e: "cancel"): void }>();

function onKey(ev: KeyboardEvent) {
  if (!props.open) return;
  if (ev.key === "Escape") emit("cancel");
}

onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));

watch(() => props.open, (v) => {
  // 防止背景滚动
  document.body.style.overflow = v ? "hidden" : "";
});
</script>

<template>
  <Teleport to="body">
    <Transition name="confirm">
      <div v-if="open" class="modal-overlay" style="z-index: 110" @click.self="emit('cancel')">
        <div class="confirm-content">
          <div class="text-base font-semibold mb-3">{{ title }}</div>
          <div class="text-[13px] mb-4" style="color: var(--text-secondary); line-height: 1.6">
            <slot />
          </div>
          <div class="flex justify-end gap-2">
            <button class="btn btn-secondary btn-sm" :disabled="busy" @click="emit('cancel')">{{ cancelText }}</button>
            <button
              class="btn btn-sm"
              :class="variant === 'danger' ? 'btn-danger' : 'btn-primary'"
              :disabled="busy"
              @click="emit('confirm')"
            >{{ busy ? '处理中…' : confirmText }}</button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.confirm-content {
  background: var(--bg-modal);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 18px 20px;
  width: 420px;
  max-width: calc(100vw - 32px);
  box-shadow: 0 24px 48px rgba(0, 0, 0, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.08);
  backdrop-filter: var(--bg-glass-blur);
  -webkit-backdrop-filter: var(--bg-glass-blur);
  animation: slideUp 220ms cubic-bezier(0.16, 1, 0.3, 1);
}
.confirm-enter-active, .confirm-leave-active { transition: opacity 0.18s ease; }
.confirm-enter-from, .confirm-leave-to { opacity: 0; }
</style>
