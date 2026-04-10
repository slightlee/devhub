// useCliHubState 工具函数测试：校验模块判定与日志通道映射语义。
import { describe, expect, it, vi } from "vitest";
import {
  isModuleKey,
  logStatusChannel,
  resolveOsName,
  shouldSurfaceLogStatus,
} from "../src/composables/useCliHubState";

vi.mock("@tauri-apps/plugin-os", () => ({
  type: vi.fn(() => "windows"),
  version: vi.fn(() => "10.0.22631"),
}));

describe("useCliHubState", () => {
  it("应识别合法模块名并拒绝非法值", () => {
    expect(isModuleKey("dashboard")).toBe(true);
    expect(isModuleKey("cli")).toBe(true);
    expect(isModuleKey("mcp")).toBe(true);
    expect(isModuleKey("skills")).toBe(true);
    expect(isModuleKey("settings")).toBe(true);
    expect(isModuleKey("unknown")).toBe(false);
    expect(isModuleKey("")).toBe(false);
  });

  it("应仅将 warn/error 级别日志上浮为可见错误", () => {
    expect(shouldSurfaceLogStatus("warn")).toBe(true);
    expect(shouldSurfaceLogStatus("error")).toBe(true);
    expect(shouldSurfaceLogStatus("info")).toBe(false);
    expect(shouldSurfaceLogStatus("success")).toBe(false);
  });

  it("应将 warn 与 error 分流到不同提示通道", () => {
    expect(logStatusChannel("warn")).toBe("warning");
    expect(logStatusChannel("error")).toBe("error");
    expect(logStatusChannel("info")).toBeNull();
    expect(logStatusChannel("success")).toBeNull();
  });

  it("Windows 内核版本为 10.0.22000+ 时应展示为 Windows 11", () => {
    expect(resolveOsName()).toBe("Windows 11 (10.0.22631)");
  });
});
