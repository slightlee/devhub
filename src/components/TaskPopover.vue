<script setup lang="ts">
// 任务浮层：显示进行中工具任务的动作类型与进度。
import type { Tool } from "../types/models";

interface Props {
  isOpen: boolean;
  hasTasks: boolean;
  taskMeta: string;
  tasks: Tool[];
}

defineProps<Props>();
</script>

<template>
  <div class="task-popover" :class="{ 'is-open': isOpen }">
    <div class="task-popover-head">
      <div>
        <div class="task-title">任务进行中</div>
        <div class="task-meta">{{ taskMeta }}</div>
      </div>
    </div>
    <div v-if="hasTasks" class="task-list compact">
      <div v-for="task in tasks" :key="task.id" class="task-item">
        <span class="task-name">
          {{ task.name }}
          {{ task.activeAction === 'uninstall' ? '卸载' : task.activeAction === 'install' ? '安装' : '更新' }}
        </span>
        <div class="task-progress">
          <div class="progress" :class="`progress-${task.activeAction || 'install'}`">
            <div class="progress-bar" :style="{ width: `${task.progress}%` }"></div>
          </div>
          <span class="progress-value">{{ Math.round(task.progress) }}%</span>
        </div>
      </div>
    </div>
    <div v-else class="task-empty">当前没有进行中的任务</div>
  </div>
</template>
