// CliHub 页面主状态编排层：聚合 settings/tool-actions/task-progress 并对外提供统一接口。
import { onBeforeUnmount, onMounted, ref } from "vue";
import { bootstrapCliHubState } from "./useCliHubBootstrap";
import { useCliHubNotifications } from "./useCliHubNotifications";
import { isModuleKey, type ModuleKey } from "./useCliHubState.helpers";
import { useSettingsState } from "./useSettingsState";
import { useTaskProgress } from "./useTaskProgress";
import { useToolActions } from "./useToolActions";

export { isModuleKey, logStatusChannel, resolveOsName, shouldSurfaceLogStatus } from "./useCliHubState.helpers";

export const useCliHubState = () => {
  const unlistenFns: Array<() => void> = [];

  const isTaskOpen = ref(false);
  const activeModule = ref<ModuleKey>("dashboard");
  const osName = ref("macOS");
  const appVersion = ref("--");

  const { addLog, dismissError, dismissWarning, lastError, lastWarning, reportActionFailure } =
    useCliHubNotifications();

  const selectModule = (value: string) => {
    if (!isModuleKey(value)) return;
    activeModule.value = value;
  };

  const { clearLogs, loadLogsDir, loadSettings, logsDir, openLogsDir, settings, updateSettings } =
    useSettingsState(addLog);

  const {
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
  } = useToolActions(addLog, reportActionFailure);

  const { activeTasks, hasTasks, summary, taskChipLabel, taskMeta, toggleTask } = useTaskProgress(
    tools,
    isTaskOpen,
  );

  onMounted(async () => {
    const listeners = await bootstrapCliHubState({
      appVersion,
      loadActionCommands,
      loadLogsDir,
      loadSettings,
      osName,
      refreshTools,
      settings,
      subscribeToolEvents,
    });
    unlistenFns.push(...listeners);
  });

  onBeforeUnmount(() => {
    unlistenFns.forEach((fn) => fn());
  });

  return {
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
  };
};
