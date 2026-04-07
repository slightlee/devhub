<script setup lang="ts">
// 仪表盘页面：汇总模块状态与运行环境信息，提供只读概览。
import { computed } from "vue";

interface Summary {
  installed: number;
  updateAvailable: number;
  notInstalled: number;
  inProgress: number;
}

interface Props {
  osName: string;
  shellConfigFile: string;
  proxyEnabled: boolean;
  proxyUrl: string;
  summary: Summary;
}

const props = defineProps<Props>();

interface ModuleRow {
  id: string;
  name: string;
  desc: string;
  status: string;
  tone?: "ok" | "warn" | "info";
}

const buildCliStatus = () => {
  const parts: string[] = [];
  if (props.summary.updateAvailable > 0) {
    parts.push(`${props.summary.updateAvailable} 项可更新`);
  }
  if (props.summary.notInstalled > 0) {
    parts.push(`${props.summary.notInstalled} 项未安装`);
  }
  if (parts.length === 0) {
    return "已是最新状态";
  }
  return parts.join(" · ");
};

const moduleRows = computed<ModuleRow[]>(() => [
  {
    id: "cli",
    name: "CLI 工具",
    desc: "安装与更新",
    status: buildCliStatus(),
    tone: props.summary.updateAvailable > 0 || props.summary.notInstalled > 0 ? "warn" : "ok",
  },
  {
    id: "mcp",
    name: "MCP",
    desc: "连接与资源（即将支持）",
    status: "暂无配置",
    tone: "info",
  },
  {
    id: "skills",
    name: "Skills",
    desc: "自动化能力（即将支持）",
    status: "暂无配置",
    tone: "info",
  },
]);

</script>

<template>
  <section class="dashboard">
    <div class="dashboard-grid">
      <section class="panel panel-wide">
        <div class="panel-head">
          <div>
            <h3>模块概览</h3>
            <p>查看各模块当前状态</p>
          </div>
        </div>
        <div class="module-list">
          <div v-for="row in moduleRows" :key="row.id" class="module-row">
            <div class="module-meta">
              <div class="module-name">{{ row.name }}</div>
              <div class="module-desc">{{ row.desc }}</div>
            </div>
            <div class="module-status" :class="row.tone">{{ row.status }}</div>
          </div>
        </div>
      </section>

      <section class="panel panel-wide">
        <div class="panel-head">
          <div>
            <h3>运行环境</h3>
            <p>系统与代理配置概览</p>
          </div>
        </div>
        <div class="info-list">
          <div class="info-row">
            <span class="info-label">系统版本</span>
            <span class="info-value">{{ osName }}</span>
          </div>
          <div class="info-row">
            <span class="info-label">Shell 配置</span>
            <span class="info-value mono">{{ shellConfigFile || "--" }}</span>
          </div>
          <div class="info-row">
            <span class="info-label">代理状态</span>
            <span class="info-value">{{ proxyEnabled ? "已启用" : "未启用" }}</span>
          </div>
          <div v-if="proxyEnabled && proxyUrl" class="info-row">
            <span class="info-label">代理地址</span>
            <span class="info-value mono">{{ proxyUrl }}</span>
          </div>
        </div>
      </section>
    </div>
  </section>
</template>
