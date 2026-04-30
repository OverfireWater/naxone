<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { ChevronDown } from "lucide-vue-next";

export interface SelectOption {
  label: string;
  value: string | number | boolean | null;
  disabled?: boolean;
}

const props = withDefaults(defineProps<{
  modelValue: string | number | boolean | null;
  options: SelectOption[];
  disabled?: boolean;
  placeholder?: string;
  fullWidth?: boolean;
  triggerClass?: string;
  menuClass?: string;
}>(), {
  disabled: false,
  placeholder: "请选择",
  fullWidth: false,
  triggerClass: "",
  menuClass: "",
});

const emit = defineEmits<{
  (e: "update:modelValue", value: string | number | boolean | null): void;
}>();

const instanceId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
  ? `rs-select-${crypto.randomUUID()}`
  : `rs-select-${Date.now()}-${Math.random().toString(36).slice(2)}`;
const open = ref(false);
const triggerRef = ref<HTMLElement | null>(null);
const menuStyle = ref<Record<string, string>>({});

const selected = computed(() => props.options.find((o) => Object.is(o.value, props.modelValue)) ?? null);
const displayLabel = computed(() => selected.value?.label ?? props.placeholder);

function close() {
  open.value = false;
}

function updateMenuPosition() {
  if (!triggerRef.value) return;
  const rect = triggerRef.value.getBoundingClientRect();
  const gap = 4;
  const viewportPadding = 8;
  const estimatedHeight = Math.min(320, Math.max(120, props.options.length * 34 + 8));
  const spaceBelow = window.innerHeight - rect.bottom - viewportPadding;
  const spaceAbove = rect.top - viewportPadding;
  const openUp = spaceBelow < Math.min(estimatedHeight, 200) && spaceAbove > spaceBelow;
  const maxHeight = Math.max(120, Math.min(320, openUp ? spaceAbove - gap : spaceBelow - gap));

  menuStyle.value = {
    position: "fixed",
    left: `${Math.max(viewportPadding, rect.left)}px`,
    top: openUp ? `${Math.max(viewportPadding, rect.top - gap)}px` : `${Math.min(window.innerHeight - viewportPadding, rect.bottom + gap)}px`,
    width: `${rect.width}px`,
    maxHeight: `${maxHeight}px`,
    transform: openUp ? "translateY(-100%)" : "none",
  };
}

function onSiblingOpen(event: Event) {
  const openedId = (event as CustomEvent<string>).detail;
  if (openedId !== instanceId) close();
}

async function toggle() {
  if (props.disabled) return;
  const nextOpen = !open.value;
  if (!nextOpen) {
    close();
    return;
  }
  window.dispatchEvent(new CustomEvent("rs-select-open", { detail: instanceId }));
  open.value = true;
  await nextTick();
  updateMenuPosition();
}

function selectOption(option: SelectOption) {
  if (option.disabled) return;
  emit("update:modelValue", option.value);
  close();
}

function onWindowClick() {
  close();
}

function onViewportChange() {
  if (open.value) updateMenuPosition();
}

window.addEventListener("rs-select-open", onSiblingOpen);

watch(open, (value) => {
  if (value) {
    window.addEventListener("click", onWindowClick);
    window.addEventListener("resize", onViewportChange);
    window.addEventListener("scroll", onViewportChange, true);
  } else {
    window.removeEventListener("click", onWindowClick);
    window.removeEventListener("resize", onViewportChange);
    window.removeEventListener("scroll", onViewportChange, true);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("rs-select-open", onSiblingOpen);
  window.removeEventListener("click", onWindowClick);
  window.removeEventListener("resize", onViewportChange);
  window.removeEventListener("scroll", onViewportChange, true);
});
</script>

<template>
  <div class="rs-select" :class="[{ 'w-full': fullWidth }, triggerClass]" @click.stop>
    <button
      ref="triggerRef"
      type="button"
      class="rs-select-trigger"
      :class="[{ 'w-full': fullWidth, disabled }, triggerClass]"
      :disabled="disabled"
      @click.stop="toggle"
    >
      <span class="truncate">{{ displayLabel }}</span>
      <ChevronDown :size="12" class="shrink-0 transition-transform" :class="{ 'rotate-180': open }" />
    </button>
    <Teleport to="body">
      <div v-if="open" class="rs-select-menu" :class="menuClass" :style="menuStyle" @click.stop>
        <button
          v-for="option in options"
          :key="`${option.label}-${String(option.value)}`"
          type="button"
          class="rs-select-option"
          :class="{ active: Object.is(option.value, modelValue), disabled: option.disabled }"
          :disabled="option.disabled"
          @click.stop="selectOption(option)"
        >
          {{ option.label }}
        </button>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.rs-select {
  position: relative;
  min-width: 0;
}

.rs-select-trigger {
  width: 100%;
  display: inline-flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  text-align: left;
  font-size: 12px;
  color: var(--text-primary);
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 6px 8px;
  outline: none;
  cursor: pointer;
  transition: border-color 150ms ease, background-color 150ms ease;
}

.rs-select-trigger:hover {
  border-color: var(--text-muted);
  background: var(--bg-hover);
}

.rs-select-trigger:focus-visible {
  border-color: var(--color-blue-light);
}

.rs-select-trigger.disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.rs-select-menu {
  z-index: 1000;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 4px;
  border-radius: 8px;
  border: 1px solid var(--border-color);
  background: var(--bg-secondary);
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
  overflow-y: auto;
}

.rs-select-option {
  border: none;
  background: transparent;
  color: var(--text-primary);
  text-align: left;
  font-size: 12px;
  padding: 6px 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 150ms ease, color 150ms ease;
}

.rs-select-option:hover,
.rs-select-option.active {
  background: var(--bg-hover);
}

.rs-select-option.disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>

