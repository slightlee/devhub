<script setup lang="ts">
// 路径详情弹窗：展示可执行路径与配置路径，并支持复制。
import { computed } from "vue";
import type { Tool } from "../types/models";

interface Props {
  show: boolean;
  tool: Tool | null;
}

interface Emits {
  (event: "close"): void;
  (event: "copy-path", payload: { path: string; label: string }): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

const isVisible = computed(() => props.show && Boolean(props.tool));
const toolName = computed(() => props.tool?.name || "");
const execPath = computed(() => props.tool?.path || "--");
const configPath = computed(() => props.tool?.configPath || "--");
const showConfigHint = computed(() => props.tool?.status === "not_installed");

const copy = (path: string, label: string) => {
  emit("copy-path", { path, label });
};
</script>

<template>
  <div v-if="isVisible" class="modal-overlay">
    <div class="modal">
      <div class="modal-header">
        <div class="modal-icon"></div>
        <div>
          <div class="modal-title">路径详情</div>
          <div class="modal-sub">{{ toolName }}</div>
        </div>
      </div>
      <div class="modal-body">
        <div class="modal-command">
          <div class="command-label path-label">
            <span>可执行文件路径</span>
            <button class="btn btn-ghost" type="button" :disabled="execPath === '--'" @click="copy(execPath, '可执行文件路径')">
              复制
            </button>
          </div>
          <pre class="command-box"><code>{{ execPath }}</code></pre>
        </div>
        <div class="modal-command">
          <div class="command-label path-label">
            <span>配置路径</span>
            <button class="btn btn-ghost" type="button" :disabled="configPath === '--'" @click="copy(configPath, '配置路径')">
              复制
            </button>
          </div>
          <pre class="command-box"><code>{{ configPath }}</code></pre>
        </div>
        <div v-if="showConfigHint" class="modal-hint">未安装时配置目录可能不存在。</div>
        <div v-else class="modal-hint">配置目录通常在首次运行时生成</div>
      </div>
      <div class="modal-actions">
        <button class="btn btn-ghost" type="button" @click="emit('close')">关闭</button>
      </div>
    </div>
  </div>
</template>
