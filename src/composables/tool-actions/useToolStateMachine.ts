// 工具状态机：维护工具状态、路径弹窗状态与 Claude PATH 后置动作。
// cspell:ignore tauri
import { invoke } from "@tauri-apps/api/core";
import { computed, ref } from "vue";
import { initialTools } from "../../data/initial-data";
import type { Tool } from "../../types/models";
import type { AddLog, ToolProgressPayload } from "./types";

interface Params {
  addLog: AddLog;
}

export const useToolStateMachine = ({ addLog }: Params) => {
  const tools = ref<Tool[]>(initialTools.map((tool) => ({ ...tool })));
  const pendingPathFix = new Set<string>();
  const pendingPathCleanup = new Set<string>();

  const showPathModal = ref(false);
  const pathToolId = ref<string | null>(null);

  const toolForPath = computed(() => {
    if (!pathToolId.value) return null;
    return tools.value.find((tool) => tool.id === pathToolId.value) || null;
  });

  const shellConfigFile = computed(() => {
    const hit = tools.value.find((tool) => tool.shellConfigFile && tool.shellConfigFile !== "--");
    return hit?.shellConfigFile || "--";
  });

  const maybeRunPostActions = (prev: Tool | null, next: Tool) => {
    if (next.id === "claude" && next.supportsPathFix) {
      if (pendingPathFix.has(next.id) && prev?.status === "installing") {
        if (["installed", "update_available"].includes(next.status)) {
          pendingPathFix.delete(next.id);
          void invoke("apply_path_fix", { toolId: next.id }).catch(() => {
            addLog("写入 PATH 失败，请手动检查配置文件。", "error");
          });
        } else if (next.status !== "installing") {
          pendingPathFix.delete(next.id);
        }
      }
      if (pendingPathCleanup.has(next.id) && prev?.status === "uninstalling") {
        if (next.status === "not_installed") {
          pendingPathCleanup.delete(next.id);
          void invoke("apply_path_cleanup", { toolId: next.id }).catch(() => {
            addLog("清理 PATH 失败，请手动检查配置文件。", "error");
          });
        } else if (next.status !== "uninstalling") {
          pendingPathCleanup.delete(next.id);
        }
      }
    }
  };

  const updateTool = (next: Tool) => {
    const index = tools.value.findIndex((tool) => tool.id === next.id);
    const prev = index >= 0 ? tools.value[index] : null;
    if (index >= 0) {
      tools.value[index] = { ...tools.value[index], ...next };
    } else {
      tools.value.push(next);
    }
    maybeRunPostActions(prev, next);
  };

  const updateToolProgress = (payload: ToolProgressPayload) => {
    const tool = tools.value.find((item) => item.id === payload.toolId);
    if (!tool) return;
    tool.progress = payload.progress;
    tool.status = payload.status;
    if (payload.status === "installing") tool.activeAction = "install";
    else if (payload.status === "updating") tool.activeAction = "update";
    else if (payload.status === "uninstalling") tool.activeAction = "uninstall";
    else tool.activeAction = undefined;
  };

  const markPendingPathFix = (toolId: string) => {
    pendingPathFix.add(toolId);
  };

  const markPendingPathCleanup = (toolId: string) => {
    pendingPathCleanup.add(toolId);
  };

  const clearPendingPathState = (toolId: string) => {
    pendingPathFix.delete(toolId);
    pendingPathCleanup.delete(toolId);
  };

  const openPath = (toolId: string) => {
    pathToolId.value = toolId;
    showPathModal.value = true;
  };

  const closePath = () => {
    showPathModal.value = false;
  };

  const copyPath = async (payload: { path: string; label: string }) => {
    const { path, label } = payload;
    if (!path || path === "--") return;
    try {
      await navigator.clipboard.writeText(path);
      addLog(`已复制${label}：${path}`, "success");
    } catch {
      addLog("复制失败，请检查系统权限", "error");
    }
  };

  return {
    clearPendingPathState,
    closePath,
    copyPath,
    markPendingPathCleanup,
    markPendingPathFix,
    openPath,
    shellConfigFile,
    showPathModal,
    toolForPath,
    tools,
    updateTool,
    updateToolProgress,
  };
};
