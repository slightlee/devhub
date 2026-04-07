// 前后端共享的领域模型与状态类型定义。
export type ToolStatus =
  | "not_installed"
  | "installed"
  | "update_available"
  | "installing"
  | "updating"
  | "uninstalling";

export type ActionType = "install" | "update" | "uninstall" | "batch_update" | "fix_path";

export type LogStatus = "info" | "success" | "warn" | "error";
export type SourceStatus = "ok" | "fail" | "unknown";

export interface Tool {
  id: string;
  name: string;
  vendor: string;
  vendorIcon: string;
  status: ToolStatus;
  currentVersion: string;
  latestVersion: string;
  path: string;
  configPath: string;
  pathNeedsSetup: boolean;
  shellConfigFile: string;
  progress: number;
  activeAction?: ActionType;
}

export interface PendingAction {
  action: ActionType;
  toolId?: string;
}

export interface SettingsState {
  autoRefreshOnLaunch: boolean;
  proxyEnabled: boolean;
  proxyUrl: string;
  logPersistenceEnabled: boolean;
  logRetentionDays: number;
}

export interface SourceCheckResult {
  overall: SourceStatus;
  npm: SourceStatus;
  claude: SourceStatus;
  checkedAt: number;
}
