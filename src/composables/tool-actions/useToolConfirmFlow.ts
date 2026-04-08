// 操作确认流程：管理确认弹窗状态、命令预览与选项逻辑。
// cspell:ignore tauri
import { invoke } from "@tauri-apps/api/core";
import { computed, ref, type Ref } from "vue";
import type { ActionType, PendingAction, Tool } from "../../types/models";
import { COMMAND_PREVIEW_UNAVAILABLE } from "./error-utils";
import type { ActionCommandsMap, AddLog } from "./types";

interface Params {
  addLog: AddLog;
  checkSources: (action?: ActionType, toolId?: string) => Promise<void>;
  tools: Ref<Tool[]>;
}

export const useToolConfirmFlow = ({ addLog, checkSources, tools }: Params) => {
  const actionCommands = ref<ActionCommandsMap>({});

  const pendingAction = ref<PendingAction | null>(null);
  const optionVisible = ref(false);
  const optionChecked = ref(false);
  const optionLabel = ref("");
  const optionHint = ref("");

  const toolNameForConfirm = computed(() => {
    if (!pendingAction.value) return "";
    if (pendingAction.value.action === "batch_update") return "全部工具";
    return tools.value.find((tool) => tool.id === pendingAction.value?.toolId)?.name || "";
  });

  const getTool = (toolId?: string) => {
    if (!toolId) return null;
    return tools.value.find((tool) => tool.id === toolId) || null;
  };

  const vendorIconForConfirm = computed(() => {
    if (!pendingAction.value || pendingAction.value.action === "batch_update") return "";
    return tools.value.find((tool) => tool.id === pendingAction.value?.toolId)?.vendorIcon || "";
  });

  const commandForConfirm = computed(() => {
    if (!pendingAction.value) return "";
    if (pendingAction.value.action === "batch_update") return "";
    const toolId = pendingAction.value.toolId;
    if (!toolId) return "";
    const action = pendingAction.value.action;
    return actionCommands.value[toolId]?.[action] || "";
  });

  const commandHintForConfirm = computed(() => {
    if (!pendingAction.value) return "";
    if (pendingAction.value.action === "batch_update") return "将按工具逐一执行更新命令。";
    const toolId = pendingAction.value.toolId;
    const action = pendingAction.value.action;
    const hasCommandPreview = Boolean(toolId && actionCommands.value[toolId]?.[action]);
    const fallbackHint = COMMAND_PREVIEW_UNAVAILABLE;
    if (toolId === "claude" && pendingAction.value.action === "install") {
      return hasCommandPreview ? "该命令会执行远程安装脚本，请确认来源可信。" : fallbackHint;
    }
    if (toolId === "claude" && pendingAction.value.action === "update") {
      return hasCommandPreview ? "将执行 claude update 以调用官方更新通道。" : fallbackHint;
    }
    if (toolId === "claude" && pendingAction.value.action === "fix_path") {
      const tool = getTool(toolId);
      if (!tool?.supportsPathFix) return "当前平台不支持 PATH 自动写入，请手动配置 PATH。";
      const configFile = tool.shellConfigFile || "--";
      return `将写入 ${configFile}（含 # devhub 标记），完成后请在终端执行 source \"${configFile}\" 或重启终端。`;
    }
    if (pendingAction.value.action === "uninstall") {
      if (toolId === "claude" && optionChecked.value) {
        return hasCommandPreview
          ? "将同时清理 PATH（仅移除 DevHub 标记行）。"
          : `${fallbackHint} 将同时清理 PATH（仅移除 DevHub 标记行）。`;
      }
      return hasCommandPreview ? "" : fallbackHint;
    }
    return hasCommandPreview ? "将执行系统命令并写入可执行文件路径。" : fallbackHint;
  });

  const loadActionCommands = async () => {
    try {
      const remote = await invoke<ActionCommandsMap>("get_action_commands");
      if (remote && typeof remote === "object") {
        actionCommands.value = remote;
      }
    } catch {
      addLog("命令预览加载失败，已使用兜底提示。", "warn");
    }
  };

  const resetConfirmState = () => {
    pendingAction.value = null;
    optionVisible.value = false;
    optionChecked.value = false;
    optionLabel.value = "";
    optionHint.value = "";
  };

  const openConfirm = (action: ActionType, toolId?: string) => {
    pendingAction.value = { action, toolId };
    optionVisible.value = false;
    optionChecked.value = false;
    optionLabel.value = "";
    optionHint.value = "";
    if (["install", "update", "batch_update"].includes(action)) {
      void checkSources(action, toolId);
    }
    if (toolId === "claude" && action === "install") {
      const tool = getTool(toolId);
      if (tool?.supportsPathFix) {
        const configFile = tool.shellConfigFile || "--";
        optionVisible.value = true;
        optionChecked.value = true;
        optionLabel.value = "安装完成后写入 PATH（推荐）";
        optionHint.value = `将写入 ${configFile}（含 # devhub 标记），便于终端直接使用 claude。`;
      }
    }
    if (toolId === "claude" && action === "uninstall") {
      const tool = getTool(toolId);
      if (tool?.supportsPathFix) {
        const configFile = tool.shellConfigFile || "--";
        optionVisible.value = true;
        optionChecked.value = false;
        optionLabel.value = "同时清理 PATH（仅移除 DevHub 标记行）";
        optionHint.value = `仅清理 ${configFile} 中包含 # devhub 的行。`;
      }
    }
  };

  const cancelConfirm = () => {
    resetConfirmState();
  };

  const toggleOption = (checked: boolean) => {
    optionChecked.value = checked;
  };

  return {
    cancelConfirm,
    commandForConfirm,
    commandHintForConfirm,
    loadActionCommands,
    openConfirm,
    optionChecked,
    optionHint,
    optionLabel,
    optionVisible,
    pendingAction,
    resetConfirmState,
    toggleOption,
    toolNameForConfirm,
    vendorIconForConfirm,
  };
};
