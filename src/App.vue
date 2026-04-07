<script setup lang="ts">
// 根界面容器：组合侧栏、顶部栏与各模块页面，并承接全局弹窗交互。
import TopBar from "./components/TopBar.vue";
import SidebarNav from "./components/SidebarNav.vue";
import TaskPopover from "./components/TaskPopover.vue";
import ToolSection from "./components/ToolSection.vue";
import ConfirmModal from "./components/ConfirmModal.vue";
import PathModal from "./components/PathModal.vue";
import SettingsPage from "./components/SettingsPage.vue";
import DashboardPage from "./components/DashboardPage.vue";
import PlaceholderPage from "./components/PlaceholderPage.vue";
import { useCliHubState } from "./composables/useCliHubState";

const {
  activeTasks,
  activeModule,
  appVersion,
  cancelConfirm,
  clearLogs,
  closePath,
  commandForConfirm,
  commandHintForConfirm,
  confirmAction,
  copyPath,
  dismissError,
  dismissWarning,
  hasTasks,
  isCheckingSources,
  isRefreshing,
  isTaskOpen,
  lastError,
  lastWarning,
  logsDir,
  openConfirm,
  openLogsDir,
  openPath,
  optionChecked,
  optionHint,
  optionLabel,
  optionVisible,
  osName,
  pendingAction,
  refreshTools,
  selectModule,
  settings,
  shellConfigFile,
  showPathModal,
  sourceLabel,
  sourceStatus,
  summary,
  taskChipLabel,
  taskMeta,
  toggleOption,
  toggleTask,
  toolForPath,
  toolNameForConfirm,
  tools,
  updateSettings,
  vendorIconForConfirm,
} = useCliHubState();
</script>

<template>
  <div class="app-root">
    <div class="window">
      <div class="shell">
        <SidebarNav :active="activeModule" @select="selectModule" />

        <div class="main-panel">
          <TopBar
            v-if="activeModule === 'cli'"
            title="CLI 工具管理"
            :is-refreshing="isRefreshing"
            :task-chip-label="taskChipLabel"
            :has-tasks="hasTasks"
            :is-task-open="isTaskOpen"
            :show-refresh="true"
            :show-batch-update="true"
            :show-task-chip="true"
            @toggle-task="toggleTask"
            @refresh="refreshTools"
            @batch-update="openConfirm('batch_update')"
          />
          <TopBar v-else-if="activeModule === 'dashboard'" title="全局概览" />
          <TopBar v-else-if="activeModule === 'mcp'" title="MCP" subtitle="连接与管理模型上下文协议" />
          <TopBar v-else-if="activeModule === 'skills'" title="Skills" subtitle="扩展能力与自动化任务" />
          <TopBar v-else title="设置" subtitle="偏好与运行策略配置" />

          <TaskPopover
            v-if="hasTasks && activeModule === 'cli'"
            :is-open="isTaskOpen"
            :has-tasks="hasTasks"
            :task-meta="taskMeta"
            :tasks="activeTasks"
          />

          <main class="content">
            <div v-if="activeModule === 'settings'" class="content-scroll settings-scroll">
              <SettingsPage
                :settings="settings"
                :logs-dir="logsDir"
                :app-version="appVersion"
                @update:settings="updateSettings"
                @clear-logs="clearLogs"
                @copy-path="copyPath"
                @open-logs-dir="openLogsDir"
              />
            </div>
            <div v-else-if="activeModule === 'cli'" class="content-scroll">
              <div v-if="lastWarning" class="callout warning">
                <div class="callout-title">提示</div>
                <div class="callout-body">{{ lastWarning.message }}</div>
                <div class="callout-actions">
                  <button class="btn btn-ghost" type="button" @click="dismissWarning">关闭</button>
                </div>
              </div>
              <div v-if="lastError" class="callout error">
                <div class="callout-title">操作失败</div>
                <div class="callout-body">{{ lastError.message }}</div>
                <div class="callout-actions">
                  <button class="btn btn-ghost" type="button" @click="openLogsDir">打开日志目录</button>
                  <button class="btn btn-ghost" type="button" @click="dismissError">关闭</button>
                </div>
              </div>
              <ToolSection
                :tools="tools"
                :summary="summary"
                @primary-action="({ toolId, action }) => openConfirm(action, toolId)"
                @uninstall="(toolId) => openConfirm('uninstall', toolId)"
                @open-path="openPath"
              />
            </div>
            <div v-else-if="activeModule === 'dashboard'" class="content-scroll">
              <DashboardPage
                :os-name="osName"
                :shell-config-file="shellConfigFile"
                :proxy-enabled="settings.proxyEnabled"
                :proxy-url="settings.proxyUrl"
                :summary="summary"
              />
            </div>
            <div v-else-if="activeModule === 'mcp'" class="content-scroll">
              <PlaceholderPage title="MCP" description="集中管理 MCP 连接与资源" />
            </div>
            <div v-else class="content-scroll">
              <PlaceholderPage title="Skills" description="管理可复用的技能与自动化流程" />
            </div>
          </main>
        </div>
      </div>

      <ConfirmModal
        :pending-action="pendingAction"
        :tool-name="toolNameForConfirm"
        :vendor-icon="vendorIconForConfirm"
        :command="commandForConfirm"
        :command-hint="commandHintForConfirm"
        :option-visible="optionVisible"
        :option-checked="optionChecked"
        :option-label="optionLabel"
        :option-hint="optionHint"
        :source-status="sourceStatus"
        :source-label="sourceLabel"
        :source-checking="isCheckingSources"
        @cancel="cancelConfirm"
        @confirm="confirmAction"
        @toggle-option="toggleOption"
      />
      <PathModal :show="showPathModal" :tool="toolForPath" @close="closePath" @copy-path="copyPath" />
    </div>
  </div>
</template>
