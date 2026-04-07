<script setup lang="ts">
// 顶部操作栏：展示模块标题并承载刷新、任务切换、一键更新等入口。
import { computed } from "vue";
interface Props {
  title: string;
  subtitle?: string;
  isRefreshing?: boolean;
  taskChipLabel?: string;
  hasTasks?: boolean;
  isTaskOpen?: boolean;
  showRefresh?: boolean;
  showBatchUpdate?: boolean;
  showTaskChip?: boolean;
}

interface Emits {
  (event: "toggle-task"): void;
  (event: "refresh"): void;
  (event: "batch-update"): void;
}

const props = defineProps<Props>();
const hasHeader = computed(
  () =>
    Boolean(props.title) ||
    Boolean(props.subtitle) ||
    props.showTaskChip ||
    props.showRefresh ||
    props.showBatchUpdate
);
const emit = defineEmits<Emits>();
</script>

<template>
  <header v-if="hasHeader" class="topbar">
    <div class="module-title">
      <div class="module-name">{{ title }}</div>
      <div v-if="subtitle" class="module-sub">{{ subtitle }}</div>
    </div>
    <div class="top-actions">
      <button
        v-if="showTaskChip && hasTasks"
        class="task-chip"
        :class="{ 'is-active': isTaskOpen }"
        type="button"
        @click="emit('toggle-task')"
      >
        <span class="task-dot active"></span>
        {{ taskChipLabel }}
      </button>
      <button
        v-if="showRefresh"
        class="btn btn-ghost"
        type="button"
        :disabled="isRefreshing"
        @click="emit('refresh')"
      >
        {{ isRefreshing ? "刷新中…" : "刷新状态" }}
      </button>
      <button
        v-if="showBatchUpdate"
        class="btn btn-cta"
        type="button"
        @click="emit('batch-update')"
      >
        一键更新
      </button>
    </div>
  </header>
</template>
