// CliHub 的告警/错误展示状态与日志上浮策略。
import { ref } from "vue";
import type { LogStatus } from "../types/models";
import { formatTime, logStatusChannel } from "./useCliHubState.helpers";

export const useCliHubNotifications = () => {
  const lastWarning = ref<{ message: string; time: string } | null>(null);
  const lastError = ref<{ message: string; time: string } | null>(null);

  const addLog = (message: string, status: LogStatus = "info", timestamp?: number) => {
    const channel = logStatusChannel(status);
    if (!channel) return;
    const time = timestamp ? formatTime(new Date(timestamp)) : formatTime(new Date());
    if (channel === "warning") {
      lastWarning.value = { message, time };
      return;
    }
    lastError.value = { message, time };
  };

  const reportActionFailure = (message: string, timestamp?: number) => {
    const time = timestamp ? formatTime(new Date(timestamp)) : formatTime(new Date());
    lastError.value = { message, time };
  };

  const dismissError = () => {
    lastError.value = null;
  };

  const dismissWarning = () => {
    lastWarning.value = null;
  };

  return {
    addLog,
    dismissError,
    dismissWarning,
    lastError,
    lastWarning,
    reportActionFailure,
  };
};
