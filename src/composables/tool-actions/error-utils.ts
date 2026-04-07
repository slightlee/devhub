// tool-actions 错误与兜底提示工具函数。
export const COMMAND_PREVIEW_UNAVAILABLE = "命令预览暂不可用，仍可继续操作。";

export const toErrorMessage = (error: unknown, fallback: string) => {
  if (typeof error === "string" && error.trim()) return error;
  if (error instanceof Error && error.message.trim()) return error.message;
  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    const message = (error as { message: string }).message.trim();
    if (message) return message;
  }
  return fallback;
};
