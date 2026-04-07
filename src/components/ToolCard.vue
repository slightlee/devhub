<script setup lang="ts">
// 单个工具卡片：展示版本/路径/状态并发出安装、更新、卸载等动作事件。
import { computed } from "vue";
import type { ActionType, Tool } from "../types/models";

interface Props {
  tool: Tool;
  index: number;
}

interface Emits {
  (event: "primary-action", payload: { toolId: string; action: ActionType }): void;
  (event: "uninstall", toolId: string): void;
  (event: "open-path", toolId: string): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

const primaryAction = computed(() => {
  switch (props.tool.status) {
    case "installing":
      return { label: "安装中", disabled: true } as const;
    case "updating":
      return { label: "更新中", disabled: true } as const;
    case "uninstalling":
      return { label: "卸载中", disabled: true } as const;
    case "not_installed":
      return { label: "安装", action: "install" as ActionType };
    case "update_available":
      return { label: "更新", action: "update" as ActionType };
    default:
      return { label: "已安装", disabled: true } as const;
  }
});

const statusLabel = computed(() => {
  switch (props.tool.status) {
    case "installed":
      return "已安装";
    case "update_available":
      return "可更新";
    case "not_installed":
      return "未安装";
    case "installing":
      return "安装中";
    case "updating":
      return "更新中";
    case "uninstalling":
      return "卸载中";
    default:
      return "未知";
  }
});

const statusClass = computed(() => {
  if (props.tool.status === "installed") return "status-installed";
  if (props.tool.status === "not_installed") return "status-idle";
  return "status-update";
});

const showStatusLabel = computed(
  () => !["installing", "updating", "uninstalling"].includes(props.tool.status),
);

const showProgress = computed(() => ["installing", "updating", "uninstalling"].includes(props.tool.status));

const progressLabel = computed(() => {
  return `${Math.round(props.tool.progress)}%`;
});

const latestVersionTip = computed(() =>
  props.tool.id === "claude" ? "参考版本来自 npm registry，可能与官方更新通道不同步" : "",
);

const showLatestTip = computed(() => props.tool.id === "claude");
const isBusy = computed(() =>
  ["installing", "updating", "uninstalling"].includes(props.tool.status),
);
const showUninstall = computed(() => ["installed", "update_available"].includes(props.tool.status));
</script>

<template>
  <article class="tool-card" :style="{ '--delay': `${index * 120}ms` }">
    <div class="tool-head">
      <div class="tool-info">
        <div class="tool-name">
          <img class="vendor-logo" :src="tool.vendorIcon" :alt="tool.vendor" :title="tool.vendor" />
          <span>{{ tool.name }}</span>
        </div>
        <div v-if="showStatusLabel" class="tool-status" :class="statusClass">{{ statusLabel }}</div>
      </div>
      <div class="tool-actions">
        <button
          v-if="showUninstall"
          class="btn btn-ghost"
          type="button"
          :disabled="isBusy"
          @click="emit('uninstall', tool.id)"
        >
          卸载
        </button>
        <button
          class="btn btn-primary"
          type="button"
          :disabled="primaryAction.disabled"
          @click="primaryAction.action && emit('primary-action', { toolId: tool.id, action: primaryAction.action })"
        >
          {{ primaryAction.label }}
        </button>
      </div>
    </div>
    <div class="tool-meta">
      <div class="meta-item">
        <span class="meta-label">当前版本</span>
        <span class="meta-value">{{ tool.currentVersion }}</span>
      </div>
      <div class="meta-item">
        <span class="meta-label">最新版本</span>
        <span class="meta-value latest">
          {{ tool.latestVersion }}
          <span v-if="showLatestTip" class="tip" :data-tip="latestVersionTip" aria-label="参考说明">i</span>
        </span>
      </div>
      <div class="meta-item">
        <span class="meta-label">可执行文件路径</span>
        <div v-if="tool.path !== '--'" class="meta-value path muted">
          <span class="truncate">{{ tool.path }}</span>
        </div>
        <span v-else class="meta-value muted">--</span>
      </div>
      <div class="meta-item">
        <span class="meta-label">配置路径</span>
        <div v-if="tool.configPath !== '--'" class="meta-value path muted">
          <span class="truncate">{{ tool.configPath }}</span>
          <button class="copy-btn" type="button" @click="emit('open-path', tool.id)">详情</button>
        </div>
        <span v-else class="meta-value muted">--</span>
      </div>
    </div>
    <div v-if="showProgress" class="progress-row">
      <div class="progress" :class="`progress-${tool.activeAction || 'install'}`">
        <div class="progress-bar" :style="{ width: `${tool.progress}%` }"></div>
      </div>
      <span class="progress-value">{{ progressLabel }}</span>
    </div>
  </article>
</template>
