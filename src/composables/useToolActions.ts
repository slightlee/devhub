// 工具动作外观层：组合 tool-actions 子模块并保持原有对外 API 兼容。
import { useToolActionExecutor } from "./tool-actions/useToolActionExecutor";
import { useToolConfirmFlow } from "./tool-actions/useToolConfirmFlow";
import { useToolEventSubscriptions } from "./tool-actions/useToolEventSubscriptions";
import { useToolSourceAndRefresh } from "./tool-actions/useToolSourceAndRefresh";
import { useToolStateMachine } from "./tool-actions/useToolStateMachine";
import type { AddLog, OnActionFailure } from "./tool-actions/types";

export const useToolActions = (addLog: AddLog, onActionFailure: OnActionFailure) => {
  const {
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
  } = useToolStateMachine({ addLog });

  const { checkSources, isCheckingSources, isRefreshing, refreshTools, sourceLabel, sourceStatus } =
    useToolSourceAndRefresh({
      addLog,
      tools,
      updateTool,
    });

  const {
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
  } = useToolConfirmFlow({
    addLog,
    checkSources,
    tools,
  });

  const { confirmAction } = useToolActionExecutor({
    addLog,
    clearPendingPathState,
    markPendingPathCleanup,
    markPendingPathFix,
    onActionFailure,
    optionChecked,
    pendingAction,
    resetConfirmState,
  });

  const { subscribeToolEvents } = useToolEventSubscriptions({
    addLog,
    onActionFailure,
    updateTool,
    updateToolProgress,
  });

  return {
    cancelConfirm,
    closePath,
    commandForConfirm,
    commandHintForConfirm,
    confirmAction,
    copyPath,
    isCheckingSources,
    isRefreshing,
    loadActionCommands,
    openConfirm,
    openPath,
    optionChecked,
    optionHint,
    optionLabel,
    optionVisible,
    pendingAction,
    refreshTools,
    shellConfigFile,
    showPathModal,
    sourceLabel,
    sourceStatus,
    subscribeToolEvents,
    toggleOption,
    toolForPath,
    toolNameForConfirm,
    tools,
    vendorIconForConfirm,
  };
};
