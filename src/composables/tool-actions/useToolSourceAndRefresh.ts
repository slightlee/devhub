// 工具来源检测与刷新流程：处理通道检查、状态刷新及并发结果去重。
// cspell:ignore tauri
import { invoke } from "@tauri-apps/api/core";
import { computed, ref, type Ref } from "vue";
import type { ActionType, SourceCheckResult, SourceStatus, Tool } from "../../types/models";
import type { AddLog } from "./types";

interface Params {
  addLog: AddLog;
  tools: Ref<Tool[]>;
  updateTool: (tool: Tool) => void;
}

export const useToolSourceAndRefresh = ({ addLog, tools, updateTool }: Params) => {
  const isRefreshing = ref(false);
  const isCheckingSources = ref(false);
  const sourceStatus = ref<SourceStatus>("unknown");
  let latestSourceCheckSeq = 0;
  let latestVersionRefreshSeq = 0;

  const sourceLabel = computed(() => {
    if (isCheckingSources.value) return "检测中";
    if (sourceStatus.value === "ok") return "正常";
    if (sourceStatus.value === "fail") return "异常";
    return "未检测";
  });

  const checkSources = async (action?: ActionType, toolId?: string) => {
    const checkSeq = ++latestSourceCheckSeq;
    isCheckingSources.value = true;
    try {
      const result = await invoke<SourceCheckResult>("check_sources", { action, toolId });
      if (checkSeq !== latestSourceCheckSeq) return;
      sourceStatus.value = result.overall || "unknown";
    } catch {
      if (checkSeq !== latestSourceCheckSeq) return;
      sourceStatus.value = "unknown";
    } finally {
      if (checkSeq === latestSourceCheckSeq) {
        isCheckingSources.value = false;
      }
    }
  };

  const refreshLatestVersions = async () => {
    const refreshSeq = ++latestVersionRefreshSeq;
    try {
      const updated = await invoke<Tool[]>("refresh_latest_versions");
      if (refreshSeq !== latestVersionRefreshSeq) return;
      if (Array.isArray(updated) && updated.length) {
        updated.forEach(updateTool);
      }
    } catch {
      if (refreshSeq !== latestVersionRefreshSeq) return;
      // 静默失败，避免打断用户刷新体验
    }
  };

  const refreshTools = async () => {
    if (isRefreshing.value) return;
    isRefreshing.value = true;
    try {
      const remoteTools = await invoke<Tool[]>("get_tools_state");
      if (Array.isArray(remoteTools) && remoteTools.length) {
        tools.value = remoteTools;
        addLog("已刷新工具状态。", "success");
        void refreshLatestVersions();
      } else {
        addLog("刷新完成，但未获取到工具状态。", "warn");
      }
    } catch {
      addLog("刷新失败，请稍后重试。", "error");
    } finally {
      isRefreshing.value = false;
    }
  };

  return {
    checkSources,
    isCheckingSources,
    isRefreshing,
    refreshTools,
    sourceLabel,
    sourceStatus,
  };
};
