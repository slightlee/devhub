// 任务进度派生状态：根据工具状态计算任务列表、统计汇总与任务面板开关。
import { computed, type Ref, watch } from "vue";
import type { Tool } from "../types/models";

export const useTaskProgress = (tools: Ref<Tool[]>, isTaskOpen: Ref<boolean>) => {
  const activeTasks = computed(() =>
    tools.value.filter((tool) => ["installing", "updating", "uninstalling"].includes(tool.status)),
  );

  const hasTasks = computed(() => activeTasks.value.length > 0);

  const estimatedMinutes = computed(() => {
    if (!hasTasks.value) return 0;
    const remaining = activeTasks.value.reduce((sum, tool) => sum + Math.max(0, 100 - tool.progress), 0);
    return Math.max(1, Math.round(remaining / 50));
  });

  const taskChipLabel = computed(() =>
    hasTasks.value ? `${activeTasks.value.length} 个任务 · ${estimatedMinutes.value} 分钟` : "无任务",
  );

  const taskMeta = computed(() =>
    hasTasks.value ? `${activeTasks.value.length} 个任务 · 预计 ${estimatedMinutes.value} 分钟` : "暂无进行中任务",
  );

  const summary = computed(() => {
    const installed = tools.value.filter((tool) => tool.status === "installed").length;
    const updateAvailable = tools.value.filter((tool) => tool.status === "update_available").length;
    const notInstalled = tools.value.filter((tool) => tool.status === "not_installed").length;
    const inProgress = activeTasks.value.length;
    return { installed, updateAvailable, notInstalled, inProgress };
  });

  const toggleTask = () => {
    if (!hasTasks.value) return;
    isTaskOpen.value = !isTaskOpen.value;
  };

  watch(hasTasks, (value) => {
    if (!value) {
      isTaskOpen.value = false;
    }
  });

  return {
    activeTasks,
    hasTasks,
    summary,
    taskChipLabel,
    taskMeta,
    toggleTask,
  };
};
