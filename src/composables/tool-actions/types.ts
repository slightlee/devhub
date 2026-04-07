// tool-actions 子模块共享类型定义。
import type { ActionType, LogStatus, Tool, ToolStatus } from "../../types/models";

export interface ToolProgressPayload {
  toolId: string;
  progress: number;
  status: ToolStatus;
}

export interface ToolUpdatedPayload {
  tool: Tool;
}

export interface ToolLogPayload {
  timestamp: number;
  message: string;
  status: LogStatus;
}

export interface ToolActionResultPayload {
  timestamp: number;
  toolId: string;
  action: string;
  success: boolean;
  message: string;
}

export interface BatchUpdateFailure {
  toolId: string;
  reason: string;
}

export interface BatchUpdateResult {
  started: string[];
  failed: BatchUpdateFailure[];
}

export type ActionCommandsMap = Record<string, Record<string, string>>;

export type AddLog = (message: string, status?: LogStatus, timestamp?: number) => void;
export type OnActionFailure = (message: string, timestamp?: number) => void;

export interface ConfirmOptionState {
  optionVisible: { value: boolean };
  optionChecked: { value: boolean };
  optionLabel: { value: string };
  optionHint: { value: string };
}

export interface PendingActionState {
  pendingAction: { value: { action: ActionType; toolId?: string } | null };
}
