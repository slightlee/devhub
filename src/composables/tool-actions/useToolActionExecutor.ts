// 动作执行层：统一处理确认后的 install/update/uninstall/batch/fix_path 调用。
// cspell:ignore tauri
import { invoke } from "@tauri-apps/api/core";
import type { Ref } from "vue";
import { toErrorMessage } from "./error-utils";
import type { ActionType, PendingAction, Tool } from "../../types/models";
import type { AddLog, BatchUpdateResult, OnActionFailure } from "./types";

interface Params {
  addLog: AddLog;
  clearPendingPathState: (toolId: string) => void;
  markPendingPathCleanup: (toolId: string) => void;
  markPendingPathFix: (toolId: string) => void;
  onActionFailure: OnActionFailure;
  optionChecked: Ref<boolean>;
  pendingAction: Ref<PendingAction | null>;
  resetConfirmState: () => void;
  tools: Ref<Tool[]>;
}

export const useToolActionExecutor = ({
  addLog,
  clearPendingPathState,
  markPendingPathCleanup,
  markPendingPathFix,
  onActionFailure,
  optionChecked,
  pendingAction,
  resetConfirmState,
  tools,
}: Params) => {
  const getTool = (toolId?: string) => {
    if (!toolId) return null;
    return tools.value.find((tool) => tool.id === toolId) || null;
  };

  const confirmAction = async () => {
    if (!pendingAction.value) return;
    const { action, toolId } = pendingAction.value;
    try {
      if (action === "batch_update") {
        const result = await invoke<BatchUpdateResult>("batch_update");
        const started = Array.isArray(result?.started) ? result.started : [];
        const failed = Array.isArray(result?.failed) ? result.failed : [];
        if (!failed.length) {
          // 全部成功或无需更新，日志由后端事件负责输出
        } else if (started.length) {
          addLog(`批量更新已启动 ${started.length} 个，${failed.length} 个启动失败。`, "warn");
        } else {
          const detail = failed
            .slice(0, 2)
            .map((item) => `${item.toolId}: ${item.reason}`)
            .join("；");
          const suffix = failed.length > 2 ? `（共 ${failed.length} 项）` : "";
          onActionFailure(`批量更新启动失败：${detail}${suffix}`);
        }
      } else if (action === "fix_path" && toolId) {
        const tool = getTool(toolId);
        if (!tool?.supportsPathFix) {
          onActionFailure("当前平台不支持 PATH 自动写入。", undefined);
        } else {
          await invoke("apply_path_fix", { toolId });
        }
      } else if (toolId) {
        const tool = getTool(toolId);
        if (action === "install" && toolId === "claude" && optionChecked.value && tool?.supportsPathFix) {
          markPendingPathFix(toolId);
        }
        if (action === "uninstall" && toolId === "claude" && optionChecked.value && tool?.supportsPathFix) {
          markPendingPathCleanup(toolId);
        }
        await invoke("start_action", { toolId, action: action as ActionType });
      }
    } catch (error) {
      if (toolId) {
        clearPendingPathState(toolId);
      }
      onActionFailure(toErrorMessage(error, "操作失败，请查看日志。"));
    }
    resetConfirmState();
  };

  return { confirmAction };
};
