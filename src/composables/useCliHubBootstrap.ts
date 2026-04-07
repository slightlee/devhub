// CliHub 启动引导流程：环境初始化、配置加载、首轮刷新与事件订阅。
// cspell:ignore tauri
import { getVersion } from "@tauri-apps/api/app";
import type { Ref } from "vue";
import { resolveOsName } from "./useCliHubState.helpers";
import type { SettingsState } from "../types/models";

interface Params {
  loadActionCommands: () => Promise<void>;
  loadLogsDir: () => Promise<void>;
  loadSettings: () => Promise<void>;
  osName: Ref<string>;
  appVersion: Ref<string>;
  refreshTools: () => Promise<void>;
  settings: Ref<SettingsState>;
  subscribeToolEvents: () => Promise<Array<() => void>>;
}

export const bootstrapCliHubState = async ({
  appVersion,
  loadActionCommands,
  loadLogsDir,
  loadSettings,
  osName,
  refreshTools,
  settings,
  subscribeToolEvents,
}: Params) => {
  osName.value = resolveOsName();
  try {
    appVersion.value = await getVersion();
  } catch {
    appVersion.value = "--";
  }

  await Promise.all([loadSettings(), loadLogsDir(), loadActionCommands()]);
  if (settings.value.autoRefreshOnLaunch) {
    await refreshTools();
  }

  return subscribeToolEvents();
};
