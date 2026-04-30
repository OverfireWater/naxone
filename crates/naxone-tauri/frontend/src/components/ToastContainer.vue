<script setup lang="ts">
import { AlertCircle, AlertTriangle, CheckCircle2, Info } from "lucide-vue-next";
import { useToast } from "../composables/useToast";

const { state, toast } = useToast();

const iconMap = {
  info: Info,
  success: CheckCircle2,
  warn: AlertTriangle,
  error: AlertCircle,
};
</script>

<template>
  <div class="toast-stack">
    <transition-group name="toast">
      <div
        v-for="t in state.items"
        :key="t.id"
        class="toast-item"
        :class="`toast-${t.level}`"
        @click="toast.dismiss(t.id)"
      >
        <component :is="iconMap[t.level]" :size="16" class="shrink-0 mt-0.5" />
        <span class="toast-msg">{{ t.msg }}</span>
        <span class="toast-x" @click.stop="toast.dismiss(t.id)">&times;</span>
      </div>
    </transition-group>
  </div>
</template>

<style scoped>
.toast-stack {
  position: fixed;
  top: 48px;
  right: 16px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 8px;
  pointer-events: none;
  max-width: 420px;
}
.toast-item {
  pointer-events: auto;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 12px 14px;
  border-radius: 8px;
  font-size: 13px;
  line-height: 1.5;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  border: 1px solid var(--border-color);
  /* 深色底 + 主文字色，保证在任意背景上都高对比 */
  background: var(--bg-secondary);
  color: var(--text-primary);
  cursor: default;
  word-break: break-word;
}
.toast-msg { flex: 1; }
.toast-x {
  cursor: pointer;
  opacity: 0.5;
  font-size: 16px;
  line-height: 1;
  margin-left: 4px;
  color: var(--text-muted);
}
.toast-x:hover { opacity: 1; }

/* 严重度只用左边图标的颜色标识，不改背景、不加边条 */
.toast-error   :is(svg) { color: #ef4444; }
.toast-warn    :is(svg) { color: #f59e0b; }
.toast-success :is(svg) { color: #22c55e; }
.toast-info    :is(svg) { color: var(--color-blue-light); }

.toast-enter-from { opacity: 0; transform: translateX(20px); }
.toast-enter-active, .toast-leave-active { transition: all 0.25s ease; }
.toast-leave-to { opacity: 0; transform: translateX(20px); }
</style>
