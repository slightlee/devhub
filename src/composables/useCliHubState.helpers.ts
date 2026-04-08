// CliHub 状态层的纯工具函数：模块判定、日志通道映射与运行环境识别。
// cspell:ignore tauri
import { type as osType, version as osVersion } from "@tauri-apps/plugin-os";
import type { LogStatus } from "../types/models";

export const MODULE_KEYS = ["dashboard", "cli", "mcp", "skills", "settings"] as const;
export type ModuleKey = (typeof MODULE_KEYS)[number];
export type NoticeChannel = "warning" | "error";

export const isModuleKey = (value: string): value is ModuleKey =>
  MODULE_KEYS.includes(value as ModuleKey);
export const logStatusChannel = (status: LogStatus): NoticeChannel | null => {
  if (status === "warn") return "warning";
  if (status === "error") return "error";
  return null;
};
export const shouldSurfaceLogStatus = (status: LogStatus) =>
  logStatusChannel(status) !== null;

export const formatTime = (date: Date) =>
  date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" });

export const detectOS = () => {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("mac")) return "macOS";
  if (ua.includes("win")) return "Windows";
  if (ua.includes("linux")) return "Linux";
  return "Unknown";
};

const normalizeWindowsVersionLabel = (label: string, versionValue: string) => {
  if (label !== "Windows") {
    return versionValue ? `${label} ${versionValue}` : label;
  }

  const majorMinor = versionValue.match(/^(\d+)\.(\d+)/);
  if (!majorMinor) {
    return versionValue ? `${label} ${versionValue}` : label;
  }

  const major = Number(majorMinor[1]);
  const minor = Number(majorMinor[2]);
  const buildMatch = versionValue.match(/^\d+\.\d+\.(\d+)/);
  const build = buildMatch ? Number(buildMatch[1]) : NaN;

  if (major === 10 && minor === 0 && Number.isFinite(build) && build >= 22000) {
    return `Windows 11 (${versionValue})`;
  }

  return versionValue ? `${label} ${versionValue}` : label;
};

export const resolveOsName = () => {
  try {
    const typeValue = osType();
    const versionValue = osVersion();
    let label = "Unknown";
    if (typeValue === "macos") label = "macOS";
    else if (typeValue === "windows") label = "Windows";
    else if (typeValue === "linux") label = "Linux";
    else if (typeValue) label = typeValue;

    if (!versionValue) {
      return label;
    }

    return normalizeWindowsVersionLabel(label, versionValue);
  } catch {
    return detectOS();
  }
};
