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
  gap: 8px;
  padding: 10px 12px;
  border-radius: 8px;
  font-size: 13px;
  line-height: 1.5;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(8px);
  border: 1px solid;
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
}
.toast-x:hover { opacity: 1; }

.toast-error {
  background: rgba(239, 68, 68, 0.14);
  border-color: rgba(239, 68, 68, 0.45);
  color: #fca5a5;
}
.toast-warn {
  background: rgba(245, 158, 11, 0.14);
  border-color: rgba(245, 158, 11, 0.45);
  color: #fcd34d;
}
.toast-success {
  background: rgba(34, 197, 94, 0.14);
  border-color: rgba(34, 197, 94, 0.45);
  color: #86efac;
}
.toast-info {
  background: rgba(99, 128, 255, 0.14);
  border-color: rgba(99, 128, 255, 0.45);
  color: #c7d2fe;
}

.toast-enter-from { opacity: 0; transform: translateX(20px); }
.toast-enter-active, .toast-leave-active { transition: all 0.25s ease; }
.toast-leave-to { opacity: 0; transform: translateX(20px); }
</style>
