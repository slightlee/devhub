// 工具事件订阅层：桥接 Tauri 事件到前端状态更新与提示回调。
// cspell:ignore tauri
import { listen } from "@tauri-apps/api/event";
import type {
  AddLog,
  OnActionFailure,
  ToolActionResultPayload,
  ToolLogPayload,
  ToolProgressPayload,
  ToolUpdatedPayload,
} from "./types";
import type { Tool } from "../../types/models";

interface Params {
  addLog: AddLog;
  onActionFailure: OnActionFailure;
  updateToolProgress: (payload: ToolProgressPayload) => void;
  updateTool: (tool: Tool) => void;
}

export const useToolEventSubscriptions = ({
  addLog,
  onActionFailure,
  updateToolProgress,
  updateTool,
}: Params) => {
  const subscribeToolEvents = async () => {
    const unlistenProgress = await listen<ToolProgressPayload>("tool-progress", (event) => {
      updateToolProgress(event.payload);
    });
    const unlistenToolUpdated = await listen<ToolUpdatedPayload>("tool-updated", (event) => {
      updateTool(event.payload.tool);
    });
    const unlistenLog = await listen<ToolLogPayload>("tool-log", (event) => {
      addLog(event.payload.message, event.payload.status, event.payload.timestamp);
    });
    const unlistenActionResult = await listen<ToolActionResultPayload>("tool-action-result", (event) => {
      if (!event.payload.success) {
        onActionFailure(event.payload.message, event.payload.timestamp);
      }
    });

    return [unlistenProgress, unlistenToolUpdated, unlistenLog, unlistenActionResult];
  };

  return { subscribeToolEvents };
};
