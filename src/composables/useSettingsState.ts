// 设置状态管理：负责设置读取/保存、日志目录操作与错误提示。
import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { LogStatus, SettingsState } from "../types/models";

const DEFAULT_LOGS_DIR = "~/.devhub/logs";

const defaultSettings = (): SettingsState => ({
  autoRefreshOnLaunch: true,
  proxyEnabled: false,
  proxyUrl: "",
  logPersistenceEnabled: true,
  logRetentionDays: 7,
});

type AddLog = (message: string, status?: LogStatus) => void;

export const useSettingsState = (addLog: AddLog) => {
  const settings = ref<SettingsState>(defaultSettings());
  const logsDir = ref(DEFAULT_LOGS_DIR);
  let saveQueue: Promise<void> = Promise.resolve();
  let proxyUrlSaveTimer: ReturnType<typeof setTimeout> | null = null;

  const enqueueSaveSettings = (next: SettingsState) => {
    saveQueue = saveQueue
      .then(async () => {
        await invoke("save_settings", { settings: next });
      })
      .catch(() => {
        addLog("设置保存失败，请重试。", "error");
      });
  };

  const scheduleProxyUrlSave = (next: SettingsState) => {
    if (proxyUrlSaveTimer) {
      clearTimeout(proxyUrlSaveTimer);
    }
    proxyUrlSaveTimer = setTimeout(() => {
      proxyUrlSaveTimer = null;
      enqueueSaveSettings(next);
    }, 400);
  };

  const updateSettings = (next: SettingsState) => {
    const prev = settings.value;
    settings.value = next;

    const onlyProxyUrlChanged =
      prev.autoRefreshOnLaunch === next.autoRefreshOnLaunch &&
      prev.proxyEnabled === next.proxyEnabled &&
      prev.logPersistenceEnabled === next.logPersistenceEnabled &&
      prev.logRetentionDays === next.logRetentionDays &&
      prev.proxyUrl !== next.proxyUrl;

    if (onlyProxyUrlChanged) {
      scheduleProxyUrlSave(next);
      return;
    }

    if (proxyUrlSaveTimer) {
      clearTimeout(proxyUrlSaveTimer);
      proxyUrlSaveTimer = null;
    }

    enqueueSaveSettings(next);
  };

  const loadLogsDir = async () => {
    try {
      logsDir.value = await invoke<string>("get_logs_dir");
    } catch {
      logsDir.value = DEFAULT_LOGS_DIR;
    }
  };

  const loadSettings = async () => {
    try {
      const remote = await invoke<SettingsState>("get_settings");
      if (remote) {
        settings.value = remote;
      }
    } catch {
      addLog("读取设置失败，已使用默认值。", "warn");
    }
  };

  const openLogsDir = async () => {
    try {
      await invoke("open_logs_dir");
      logsDir.value = await invoke<string>("get_logs_dir");
    } catch {
      addLog("打开日志目录失败，请手动检查路径。", "error");
    }
  };

  const clearLogs = async () => {
    try {
      await invoke("clear_logs");
      addLog("已清理日志文件。", "success");
    } catch {
      addLog("清理日志失败，请重试。", "error");
    }
  };

  return {
    clearLogs,
    loadLogsDir,
    loadSettings,
    logsDir,
    openLogsDir,
    settings,
    updateSettings,
  };
};
