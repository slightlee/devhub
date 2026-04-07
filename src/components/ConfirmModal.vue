<script setup lang="ts">
// 操作确认弹窗：展示动作描述、命令预览、来源状态与可选项并触发确认事件。
import { computed } from "vue";
import type { PendingAction, SourceStatus } from "../types/models";

interface Props {
  pendingAction: PendingAction | null;
  toolName: string;
  vendorIcon?: string;
  command?: string;
  commandHint?: string;
  optionVisible?: boolean;
  optionChecked?: boolean;
  optionLabel?: string;
  optionHint?: string;
  sourceStatus?: SourceStatus;
  sourceLabel?: string;
  sourceChecking?: boolean;
}

interface Emits {
  (event: "cancel"): void;
  (event: "confirm"): void;
  (event: "toggle-option", checked: boolean): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

const confirmTitle = computed(() => {
  if (!props.pendingAction) return "";
  const action = props.pendingAction.action;
  if (action === "batch_update") return "确认一键更新";
  if (action === "install") return "确认安装";
  if (action === "update") return "确认更新";
  if (action === "fix_path") return "确认修复 PATH";
  return "确认卸载";
});

const confirmDescription = computed(() => {
  if (!props.pendingAction) return "";
  const action = props.pendingAction.action;
  if (action === "batch_update") return "将对所有可更新工具执行更新。";
  if (action === "install") return "将下载并写入可执行文件路径。";
  if (action === "update") return "更新会覆盖旧版本，配置保持不变。";
  if (action === "fix_path") return "将把 ~/.local/bin 写入你的 shell 配置文件。";
  return "将卸载已安装的工具文件。";
});

const confirmButtonLabel = computed(() => {
  if (!props.pendingAction) return "确认";
  if (props.pendingAction.action === "uninstall") return "确认卸载";
  if (props.pendingAction.action === "batch_update") return "开始更新";
  if (props.pendingAction.action === "fix_path") return "确认写入";
  return "确认";
});

const confirmIsDanger = computed(() => props.pendingAction?.action === "uninstall");

const command = computed(() => props.command || "");
const commandHint = computed(() => props.commandHint || "");
const showCommand = computed(() => Boolean(command.value));
const showOption = computed(() => Boolean(props.optionVisible));
const optionChecked = computed(() => Boolean(props.optionChecked));
const optionLabel = computed(() => props.optionLabel || "");
const optionHint = computed(() => props.optionHint || "");
const vendorIcon = computed(() => props.vendorIcon || "");
const sourceLabel = computed(() => props.sourceLabel || "未检测");
const showSourceStatus = computed(() =>
  ["install", "update", "batch_update"].includes(props.pendingAction?.action || ""),
);
const sourceStatusClass = computed(() => {
  if (props.sourceChecking) return "status-warn";
  if (props.sourceStatus === "ok") return "status-ok";
  if (props.sourceStatus === "fail") return "status-fail";
  return "status-muted";
});

const toggleOption = (event: Event) => {
  const target = event.target as HTMLInputElement | null;
  emit("toggle-option", Boolean(target?.checked));
};
</script>

<template>
  <div v-if="pendingAction" class="modal-overlay">
    <div class="modal">
      <div class="modal-header">
        <div class="modal-icon" :class="{ danger: confirmIsDanger }">
          <img v-if="vendorIcon" class="modal-vendor" :src="vendorIcon" :alt="toolName" />
        </div>
        <div>
          <div class="modal-title">{{ confirmTitle }}</div>
          <div class="modal-sub">{{ toolName }}</div>
        </div>
      </div>
      <div class="modal-body">
        <div class="modal-text">{{ confirmDescription }}</div>
        <div v-if="commandHint" class="modal-hint">{{ commandHint }}</div>
        <div v-if="showSourceStatus" class="modal-status">
          <span class="modal-status-label">通道可达性</span>
          <span class="modal-status-value" :class="sourceStatusClass">{{ sourceLabel }}</span>
        </div>
        <div v-if="showCommand" class="modal-command">
          <div class="command-label">将执行命令</div>
          <pre class="command-box"><code>{{ command }}</code></pre>
        </div>
        <div v-if="showOption" class="modal-option">
          <label class="option-row">
            <input class="option-checkbox" type="checkbox" :checked="optionChecked" @change="toggleOption" />
            <span>{{ optionLabel }}</span>
          </label>
          <div v-if="optionHint" class="option-hint">{{ optionHint }}</div>
        </div>
      </div>
      <div class="modal-actions">
        <button class="btn btn-ghost" type="button" @click="emit('cancel')">取消</button>
        <button
          class="btn"
          :class="confirmIsDanger ? 'btn-danger' : 'btn-primary'"
          type="button"
          @click="emit('confirm')"
        >
          {{ confirmButtonLabel }}
        </button>
      </div>
    </div>
  </div>
</template>
