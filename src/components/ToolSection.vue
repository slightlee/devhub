<script setup lang="ts">
// 工具区域容器：渲染工具卡片列表与顶部状态汇总条。
import type { ActionType, Tool } from "../types/models";
import ToolCard from "./ToolCard.vue";

interface Summary {
  installed: number;
  updateAvailable: number;
  notInstalled: number;
  inProgress: number;
}

interface Props {
  tools: Tool[];
  summary: Summary;
}

interface Emits {
  (event: "primary-action", payload: { toolId: string; action: ActionType }): void;
  (event: "uninstall", toolId: string): void;
  (event: "open-path", toolId: string): void;
}

defineProps<Props>();
const emit = defineEmits<Emits>();
</script>

<template>
  <section class="tools">
    <div class="summary-strip">
      <div class="summary-bar">
        <span class="summary-item"><span class="summary-dot ok"></span>已安装 {{ summary.installed }}</span>
        <span class="summary-item"><span class="summary-dot warn"></span>可更新 {{ summary.updateAvailable }}</span>
        <span class="summary-item"><span class="summary-dot info"></span>未安装 {{ summary.notInstalled }}</span>
        <span class="summary-item"><span class="summary-dot active"></span>进行中 {{ summary.inProgress }}</span>
      </div>
    </div>

    <div class="tool-grid-scroll">
      <div class="tool-grid">
        <ToolCard
          v-for="(tool, index) in tools"
          :key="tool.id"
          :tool="tool"
          :index="index"
          @primary-action="emit('primary-action', $event)"
          @uninstall="emit('uninstall', $event)"
          @open-path="emit('open-path', $event)"
        />
      </div>
    </div>
  </section>
</template>
